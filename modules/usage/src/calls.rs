//! Calls to the models, read back from the agents' logs, duplicates removed.
//!
//! Claude Code: `~/.claude/projects/<project>/<session>.jsonl` and, for sub-agents,
//! `<project>/<session>/subagents/[workflows/<id>/]agent-<id>.jsonl`. One `assistant` line per
//! content block: the same call (`requestId`) comes back several times, and a resumed session copies
//! the history of the previous one into a new file; it is counted once (the most
//! complete occurrence). Effort: `perTurnEffort`, otherwise the line's `effort`; absent: unknown.
//!
//! Codex: `~/.codex/sessions/YYYY/MM/DD/rollout-….jsonl`. One call per `token_usage_record`
//! (`response_id`); the old files do not have any: one call per `token_count` event whose
//! cumulative total moved (the same event is repeated). Model and effort: the `turn_context` of the
//! turn. Sub-agent: `session_meta.source.subagent`.
//!
//! Separate tokens: input outside the cache, cache read, cache written, output, and reasoning (included
//! in the output, for both).

use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// UTC seconds.
    pub t: i64,
    pub agent: &'static str,
    pub session: String,
    pub project: String,
    pub model: String,
    pub effort: Option<String>,
    pub subagent: bool,
    pub tokens: Tokens,
    /// Deduplication key (`requestId`, `response_id`…), specific to the agent.
    pub key: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    /// Included in `output`.
    pub reasoning: u64,
}

impl Tokens {
    pub fn total(&self) -> u64 {
        self.input + self.cache_read + self.cache_write + self.output
    }

    pub fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.cache_read += o.cache_read;
        self.cache_write += o.cache_write;
        self.output += o.output;
        self.reasoning += o.reasoning;
    }

    pub fn to_json(self) -> Value {
        json!({
            "input": self.input,
            "cache_read": self.cache_read,
            "cache_write": self.cache_write,
            "output": self.output,
            "reasoning": self.reasoning,
            "total": self.total(),
        })
    }
}

impl Call {
    pub fn to_json(&self) -> Value {
        json!({
            "time": crate::time::iso(self.t),
            "agent": self.agent,
            "session": self.session,
            "project": self.project,
            "model": self.model,
            "effort": self.effort,
            "subagent": self.subagent,
            "input": self.tokens.input,
            "cache_read": self.tokens.cache_read,
            "cache_write": self.tokens.cache_write,
            "output": self.tokens.output,
            "reasoning": self.tokens.reasoning,
        })
    }
}

/// Where the logs are.
#[derive(Debug, Clone)]
pub struct Sources {
    pub claude: PathBuf,
    pub codex: PathBuf,
}

impl Sources {
    /// `CLAUDE_CONFIG_DIR` (otherwise `~/.claude`) and `CODEX_HOME` (otherwise `~/.codex`), like the tools.
    pub fn from_env(home: &Path) -> Self {
        let dir = |var: &str, def: &str| {
            std::env::var_os(var)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(def))
        };
        Self {
            claude: dir("CLAUDE_CONFIG_DIR", ".claude").join("projects"),
            codex: dir("CODEX_HOME", ".codex").join("sessions"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

/// The calls of `[since, until)`, duplicates removed, sorted by time.
pub fn load(src: &Sources, agent: Option<Agent>, since: i64, until: i64) -> Vec<Call> {
    let mut all = Vec::new();
    if agent != Some(Agent::Codex) {
        for f in jsonl_files(&src.claude, since) {
            all.extend(parse_claude_file(&f, &src.claude));
        }
    }
    if agent != Some(Agent::Claude) {
        for f in jsonl_files(&src.codex, since) {
            all.extend(parse_codex_file(&f));
        }
    }
    let mut calls = dedup(all);
    calls.retain(|c| c.t >= since && c.t < until);
    calls.sort_by(|a, b| a.t.cmp(&b.t).then_with(|| a.key.cmp(&b.key)));
    calls
}

/// The `.jsonl` files under `root`, except those that have not moved since `since` (no call after).
fn jsonl_files(root: &Path, since: i64) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let p = e.path();
            if ft.is_dir() {
                stack.push(p);
            } else if ft.is_file() && p.extension().is_some_and(|x| x == "jsonl") {
                let recent = e
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                    .is_none_or(|d| d.as_secs() as i64 >= since);
                if recent {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}

/// One occurrence per (agent, key): the most complete (the most tokens), on a tie the first.
pub fn dedup(calls: Vec<Call>) -> Vec<Call> {
    let mut seen: HashMap<(&'static str, String), usize> = HashMap::new();
    let mut out: Vec<Call> = Vec::with_capacity(calls.len());
    for c in calls {
        match seen.get(&(c.agent, c.key.clone())) {
            Some(&i) => {
                let kept = &mut out[i];
                // A copy in a sub-agent's file says that it is one.
                let sub = kept.subagent || c.subagent;
                if c.tokens.total() > kept.tokens.total() {
                    *kept = c;
                }
                kept.subagent = sub;
            }
            None => {
                seen.insert((c.agent, c.key.clone()), out.len());
                out.push(c);
            }
        }
    }
    out
}

fn base_name(p: &str) -> String {
    Path::new(p)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn lines(path: &Path) -> impl Iterator<Item = String> {
    let reader = File::open(path)
        .ok()
        .map(|f| BufReader::with_capacity(1 << 20, f));
    reader
        .into_iter()
        .flat_map(|r| r.lines().map_while(Result::ok))
}

// --- Claude Code ---

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    timestamp: Option<String>,
    request_id: Option<String>,
    session_id: Option<String>,
    uuid: Option<String>,
    #[serde(default)]
    is_sidechain: bool,
    cwd: Option<String>,
    effort: Option<String>,
    per_turn_effort: Option<String>,
    message: Option<ClaudeMessage>,
}

#[derive(Deserialize)]
struct ClaudeMessage {
    id: Option<String>,
    model: Option<String>,
    usage: Option<ClaudeUsage>,
}

#[derive(Deserialize, Default)]
struct ClaudeUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    output_tokens_details: Option<ClaudeOutputDetails>,
}

#[derive(Deserialize)]
struct ClaudeOutputDetails {
    #[serde(default)]
    thinking_tokens: u64,
}

pub fn parse_claude_file(path: &Path, root: &Path) -> Vec<Call> {
    let rel = path.strip_prefix(root).unwrap_or(path);
    let in_subagents = rel.components().any(|c| c.as_os_str() == "subagents");
    let project_dir = rel
        .components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut out = Vec::new();
    for line in lines(path) {
        if !line.contains("\"assistant\"") {
            continue;
        }
        if let Some(c) = parse_claude_line(&line, in_subagents, &project_dir) {
            out.push(c);
        }
    }
    out
}

fn parse_claude_line(line: &str, in_subagents: bool, project_dir: &str) -> Option<Call> {
    let l: ClaudeLine = serde_json::from_str(line).ok()?;
    if l.kind.as_deref() != Some("assistant") {
        return None;
    }
    let msg = l.message?;
    let model = msg.model?;
    // `<synthetic>`: message made up by the tool, no call to the model.
    if model.starts_with('<') {
        return None;
    }
    let u = msg.usage?;
    let key = l.request_id.or(msg.id).or(l.uuid)?;
    let t = crate::time::parse_iso(l.timestamp.as_deref()?)?;
    Some(Call {
        t,
        agent: "claude",
        session: l.session_id.unwrap_or_default(),
        project: l
            .cwd
            .as_deref()
            .map(base_name)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| project_dir.to_string()),
        model,
        effort: l.per_turn_effort.or(l.effort),
        subagent: in_subagents || l.is_sidechain,
        tokens: Tokens {
            input: u.input_tokens,
            cache_read: u.cache_read_input_tokens,
            cache_write: u.cache_creation_input_tokens,
            output: u.output_tokens,
            reasoning: u.output_tokens_details.map_or(0, |d| d.thinking_tokens),
        },
        key,
    })
}

// --- Codex ---

#[derive(Deserialize)]
struct CodexLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    timestamp: Option<String>,
    payload: Option<Value>,
}

#[derive(Default, Clone, Copy)]
struct CodexUsage {
    input: u64,
    cached: u64,
    cache_write: u64,
    output: u64,
    reasoning: u64,
    total: u64,
}

fn codex_usage(v: &Value) -> Option<CodexUsage> {
    let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    v.is_object().then(|| CodexUsage {
        input: n("input_tokens"),
        cached: n("cached_input_tokens"),
        cache_write: n("cache_write_input_tokens"),
        output: n("output_tokens"),
        reasoning: n("reasoning_output_tokens"),
        total: n("total_tokens"),
    })
}

impl CodexUsage {
    /// `input_tokens` also counts the cache read: we remove it to get the input outside the cache.
    fn tokens(self) -> Tokens {
        Tokens {
            input: self.input.saturating_sub(self.cached),
            cache_read: self.cached,
            cache_write: self.cache_write,
            output: self.output,
            reasoning: self.reasoning,
        }
    }
}

pub fn parse_codex_file(path: &Path) -> Vec<Call> {
    let mut session = String::new();
    let mut project = String::new();
    let mut subagent = false;
    let mut current: (String, Option<String>) = (String::new(), None);
    let mut turns: HashMap<String, (String, Option<String>)> = HashMap::new();
    let mut records = Vec::new();
    let mut counts = Vec::new();
    let mut last_total: Option<u64> = None;
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    for line in lines(path) {
        if !(line.contains("token_")
            || line.contains("turn_context")
            || line.contains("session_meta"))
        {
            continue;
        }
        let Ok(l) = serde_json::from_str::<CodexLine>(&line) else {
            continue;
        };
        let Some(p) = l.payload else { continue };
        let t = l.timestamp.as_deref().and_then(crate::time::parse_iso);
        match l.kind.as_deref() {
            Some("session_meta") => {
                session = s(&p, "id")
                    .or_else(|| s(&p, "session_id"))
                    .unwrap_or_default();
                project = s(&p, "cwd").map(|c| base_name(&c)).unwrap_or_default();
                subagent = p.get("source").and_then(|v| v.get("subagent")).is_some();
            }
            Some("turn_context") => {
                current = (s(&p, "model").unwrap_or_default(), s(&p, "effort"));
                if let Some(id) = s(&p, "turn_id") {
                    turns.insert(id, current.clone());
                }
            }
            Some("token_usage_record") => {
                let (Some(t), Some(u)) = (t, p.get("usage").and_then(codex_usage)) else {
                    continue;
                };
                let turn = s(&p, "turn_id");
                let (model, effort) = turn
                    .as_ref()
                    .and_then(|id| turns.get(id))
                    .unwrap_or(&current)
                    .clone();
                let key = s(&p, "response_id").unwrap_or_else(|| {
                    format!("{session}:{}:{}", turn.unwrap_or_default(), records.len())
                });
                records.push((t, model, effort, u.tokens(), key));
            }
            Some("event_msg") if p.get("type").and_then(Value::as_str) == Some("token_count") => {
                let Some(info) = p.get("info").filter(|i| i.is_object()) else {
                    continue;
                };
                let (Some(t), Some(last), Some(total)) = (
                    t,
                    info.get("last_token_usage").and_then(codex_usage),
                    info.get("total_token_usage").and_then(codex_usage),
                ) else {
                    continue;
                };
                // The same event is repeated (limits update): total unchanged, no call.
                if last_total == Some(total.total) {
                    continue;
                }
                last_total = Some(total.total);
                let (model, effort) = current.clone();
                counts.push((
                    t,
                    model,
                    effort,
                    last.tokens(),
                    format!("{session}:{}", total.total),
                ));
            }
            _ => {}
        }
    }
    // The recent files have both: the records (with response_id) are authoritative.
    let picked = if records.is_empty() { counts } else { records };
    picked
        .into_iter()
        .map(|(t, model, effort, tokens, key)| Call {
            t,
            agent: "codex",
            session: session.clone(),
            project: project.clone(),
            model,
            effort,
            subagent,
            tokens,
            key,
        })
        .collect()
}
