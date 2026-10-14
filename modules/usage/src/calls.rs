//! Appels aux modèles, relus dans les journaux des agents, doublons retirés.
//!
//! Claude Code : `~/.claude/projects/<projet>/<session>.jsonl` et, pour les sous-agents,
//! `<projet>/<session>/subagents/[workflows/<id>/]agent-<id>.jsonl`. Une ligne `assistant` par bloc
//! de contenu : le même appel (`requestId`) revient plusieurs fois, et une session reprise recopie
//! l'historique de la précédente dans un nouveau fichier ; on le compte une fois (l'occurrence la
//! plus complète). Effort : `perTurnEffort`, sinon `effort` de la ligne ; absent : inconnu.
//!
//! Codex : `~/.codex/sessions/AAAA/MM/JJ/rollout-….jsonl`. Un appel par `token_usage_record`
//! (`response_id`) ; les anciens fichiers n'en ont pas : un appel par événement `token_count` dont le
//! total cumulé a bougé (le même événement est répété). Modèle et effort : le `turn_context` du
//! tour. Sous-agent : `session_meta.source.subagent`.
//!
//! Tokens séparés : entrée hors cache, cache lu, cache écrit, sortie, et raisonnement (compris
//! dans la sortie, chez les deux).

use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// Secondes UTC.
    pub t: i64,
    pub agent: &'static str,
    pub session: String,
    pub project: String,
    pub model: String,
    pub effort: Option<String>,
    pub subagent: bool,
    pub tokens: Tokens,
    /// Clé de dédoublonnage (`requestId`, `response_id`…), propre à l'agent.
    pub key: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tokens {
    pub input: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub output: u64,
    /// Compris dans `output`.
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

/// Où sont les journaux.
#[derive(Debug, Clone)]
pub struct Sources {
    pub claude: PathBuf,
    pub codex: PathBuf,
}

impl Sources {
    /// `CLAUDE_CONFIG_DIR` (sinon `~/.claude`) et `CODEX_HOME` (sinon `~/.codex`), comme les outils.
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

/// Les appels de `[since, until)`, doublons retirés, triés par heure.
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

/// Les `.jsonl` sous `root`, sauf ceux qui n'ont pas bougé depuis `since` (aucun appel après).
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

/// Une occurrence par (agent, clé) : la plus complète (le plus de tokens), à égalité la première.
pub fn dedup(calls: Vec<Call>) -> Vec<Call> {
    let mut seen: HashMap<(&'static str, String), usize> = HashMap::new();
    let mut out: Vec<Call> = Vec::with_capacity(calls.len());
    for c in calls {
        match seen.get(&(c.agent, c.key.clone())) {
            Some(&i) => {
                let kept = &mut out[i];
                // Une copie dans le fichier d'un sous-agent dit qu'il en est un.
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
    // `<synthetic>` : message fabriqué par l'outil, aucun appel au modèle.
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
    /// `input_tokens` compte aussi le cache lu : on le retire pour avoir l'entrée hors cache.
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
                // Le même événement est répété (mise à jour des limites) : total inchangé, pas d'appel.
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
    // Les fichiers récents ont les deux : les enregistrements (avec response_id) font foi.
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
