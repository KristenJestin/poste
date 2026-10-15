//! 5 h windows and their indicators.
//!
//! A window comes from the history of the readings: the session limit ("Session (5-hour)",
//! "5h window") gives its reset R, the window is [R - 5 h, R). Peak percentage, and
//! time of the first reading at 100% (cap reached). Without history (before it existed, or a
//! gap), the window is estimated from the calls: it starts at the first call that falls in
//! no window (at the hour sharp below for Claude, whose resets fall on the hour),
//! and lasts 5 h; marked `estimated`, without a percentage.
//!
//! The calls are only those of this machine's logs: a quota started elsewhere counts in
//! the percentage but not in the calls.

use crate::calls::{Call, Tokens};
use crate::history::Entry;
use crate::time;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};

pub const FIVE_HOURS: i64 = 5 * 3600;
/// Two resets less than 2 min apart: the same window.
const SAME_RESET: i64 = 120;
/// Time slice to count parallel sessions.
const SLOT: i64 = 300;

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub agent: String,
    pub start: i64,
    pub end: i64,
    pub estimated: bool,
    pub peak_percent: Option<f64>,
    /// Time of the first reading at 100%.
    pub capped_at: Option<i64>,
}

/// The 5 h limit of an agent, from its label.
pub fn is_session_limit(label: &str) -> bool {
    let l = label.to_lowercase();
    !l.contains("week")
        && !l.contains("month")
        && (l.contains("5-hour")
            || l.contains("5h")
            || l.contains("5 h")
            || l.starts_with("session"))
}

/// The windows known from the history, sorted by agent then start.
pub fn history_windows(entries: &[Entry]) -> Vec<Window> {
    let mut out: Vec<Window> = Vec::new();
    for e in entries {
        let Some(t) = time::parse_iso(&e.t) else {
            continue;
        };
        for l in e.limits.iter().filter(|l| is_session_limit(&l.label)) {
            let Some(reset) = time::parse_iso(&l.resets_at) else {
                continue;
            };
            let i = match out
                .iter()
                .position(|w| w.agent == e.agent && (w.end - reset).abs() <= SAME_RESET)
            {
                Some(i) => i,
                None => {
                    out.push(Window {
                        agent: e.agent.clone(),
                        start: reset - FIVE_HOURS,
                        end: reset,
                        estimated: false,
                        peak_percent: None,
                        capped_at: None,
                    });
                    out.len() - 1
                }
            };
            let w = &mut out[i];
            w.peak_percent = Some(w.peak_percent.map_or(l.percent, |p| p.max(l.percent)));
            if l.percent >= 100.0 && w.capped_at.is_none_or(|c| t < c) {
                w.capped_at = Some(t);
            }
        }
    }
    out.sort_by(|a, b| a.agent.cmp(&b.agent).then(a.start.cmp(&b.start)));
    // A reset that slides from one reading to the next by more than SAME_RESET gives two windows
    // that overlap: the next one starts at the end of the previous one, so that a call
    // counts in only one.
    for i in 1..out.len() {
        if out[i].agent == out[i - 1].agent && out[i].start < out[i - 1].end {
            out[i].start = out[i - 1].end;
        }
    }
    out
}

/// Estimated windows for the calls that no window of the history covers.
pub fn estimate(calls: &[Call], known: &[Window]) -> Vec<Window> {
    let mut out: Vec<Window> = Vec::new();
    for c in calls {
        let mine = |w: &&Window| w.agent == c.agent;
        if known
            .iter()
            .filter(mine)
            .any(|w| c.t >= w.start && c.t < w.end)
        {
            continue;
        }
        if out
            .iter()
            .filter(mine)
            .any(|w| c.t >= w.start && c.t < w.end)
        {
            continue;
        }
        let mut start = if c.agent == "claude" {
            c.t - c.t.rem_euclid(3600)
        } else {
            c.t
        };
        // No overlap with a known or already estimated window.
        for w in known.iter().chain(out.iter()).filter(mine) {
            if w.start <= c.t && w.end > start {
                start = start.max(w.end);
            }
        }
        let mut end = start + FIVE_HOURS;
        for w in known.iter().filter(mine) {
            if w.start > start {
                end = end.min(w.start);
            }
        }
        out.push(Window {
            agent: c.agent.to_string(),
            start,
            end,
            estimated: true,
            peak_percent: None,
            capped_at: None,
        });
    }
    out
}

/// All the windows (history and estimated) that cross [since, until), sorted by start.
pub fn windows(
    entries: &[Entry],
    calls: &[Call],
    agent: Option<&str>,
    since: i64,
    until: i64,
) -> Vec<Window> {
    let mut known = history_windows(entries);
    known.retain(|w| agent.is_none_or(|a| a == w.agent));
    let mut all = estimate(calls, &known);
    all.extend(known);
    all.retain(|w| w.end > since && w.start < until);
    all.sort_by(|a, b| a.start.cmp(&b.start).then(a.agent.cmp(&b.agent)));
    all
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Part {
    pub calls: usize,
    pub tokens: Tokens,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    pub window: Window,
    pub calls: usize,
    pub subagent_calls: usize,
    pub sessions: usize,
    /// The most sessions that made a call in the same 5 min slice.
    pub parallel_max: usize,
    pub tokens: Tokens,
    pub by_effort: BTreeMap<String, Part>,
    pub by_model: BTreeMap<String, Part>,
}

pub fn effort_name(e: &Option<String>) -> String {
    e.clone().unwrap_or_else(|| "unknown".into())
}

pub fn stats(w: &Window, calls: &[Call]) -> Stats {
    let mine: Vec<&Call> = calls
        .iter()
        .filter(|c| c.agent == w.agent && c.t >= w.start && c.t < w.end)
        .collect();
    let mut tokens = Tokens::default();
    let mut by_effort: BTreeMap<String, Part> = BTreeMap::new();
    let mut by_model: BTreeMap<String, Part> = BTreeMap::new();
    let mut sessions = HashSet::new();
    let mut slots: BTreeMap<i64, HashSet<&str>> = BTreeMap::new();
    for c in &mine {
        tokens.add(&c.tokens);
        for (map, k) in [
            (&mut by_effort, effort_name(&c.effort)),
            (&mut by_model, c.model.clone()),
        ] {
            let p = map.entry(k).or_default();
            p.calls += 1;
            p.tokens.add(&c.tokens);
        }
        sessions.insert(c.session.as_str());
        slots
            .entry(c.t.div_euclid(SLOT))
            .or_default()
            .insert(c.session.as_str());
    }
    Stats {
        window: w.clone(),
        calls: mine.len(),
        subagent_calls: mine.iter().filter(|c| c.subagent).count(),
        sessions: sessions.len(),
        parallel_max: slots.values().map(HashSet::len).max().unwrap_or(0),
        tokens,
        by_effort,
        by_model,
    }
}

fn pct(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        (part as f64 * 1000.0 / total as f64).round() / 10.0
    }
}

fn parts_json(map: &BTreeMap<String, Part>, calls: usize, output: u64) -> Value {
    let mut m = Map::new();
    for (k, p) in map {
        m.insert(
            k.clone(),
            json!({
                "calls": p.calls,
                "calls_share": pct(p.calls as u64, calls as u64),
                "output": p.tokens.output,
                "output_share": pct(p.tokens.output, output),
                "total": p.tokens.total(),
            }),
        );
    }
    Value::Object(m)
}

fn per_point(t: &Tokens, points: f64) -> Value {
    if points <= 0.0 {
        return Value::Null;
    }
    let d = |v: u64| (v as f64 / points).round() as u64;
    json!({
        "input": d(t.input),
        "cache_read": d(t.cache_read),
        "cache_write": d(t.cache_write),
        "output": d(t.output),
        "reasoning": d(t.reasoning),
        "total": d(t.total()),
    })
}

impl Stats {
    /// Quota points whose tokens can be related: the peak of a recorded window.
    fn points(&self) -> f64 {
        if self.window.estimated {
            0.0
        } else {
            self.window.peak_percent.unwrap_or(0.0)
        }
    }

    pub fn to_json(&self) -> Value {
        let w = &self.window;
        json!({
            "agent": w.agent,
            "start": time::iso(w.start),
            "end": time::iso(w.end),
            "estimated": w.estimated,
            "peak_percent": w.peak_percent,
            "capped_after_minutes": w.capped_at.map(|c| (c - w.start) / 60),
            "calls": self.calls,
            "subagent_calls": self.subagent_calls,
            "sessions": self.sessions,
            "parallel_sessions_max": self.parallel_max,
            "tokens": self.tokens.to_json(),
            "tokens_per_point": per_point(&self.tokens, self.points()),
            "effort": parts_json(&self.by_effort, self.calls, self.tokens.output),
            "models": parts_json(&self.by_model, self.calls, self.tokens.output),
        })
    }
}

/// The indicators of several windows gathered (for `compare`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    pub windows: usize,
    pub estimated: usize,
    pub capped: usize,
    pub capped_after_sum: i64,
    pub calls: usize,
    pub subagent_calls: usize,
    pub parallel_max_sum: usize,
    pub tokens: Tokens,
    /// Tokens and points of the recorded windows only (for the tokens per point).
    pub measured_tokens: Tokens,
    pub points: f64,
    pub by_effort: BTreeMap<String, Part>,
    pub by_model: BTreeMap<String, Part>,
}

pub fn summarize(all: &[Stats]) -> Summary {
    let mut s = Summary::default();
    for st in all {
        s.windows += 1;
        s.estimated += usize::from(st.window.estimated);
        if let Some(c) = st.window.capped_at {
            s.capped += 1;
            s.capped_after_sum += c - st.window.start;
        }
        s.calls += st.calls;
        s.subagent_calls += st.subagent_calls;
        s.parallel_max_sum += st.parallel_max;
        s.tokens.add(&st.tokens);
        if st.points() > 0.0 {
            s.measured_tokens.add(&st.tokens);
            s.points += st.points();
        }
        for (dst, src) in [
            (&mut s.by_effort, &st.by_effort),
            (&mut s.by_model, &st.by_model),
        ] {
            for (k, p) in src {
                let d = dst.entry(k.clone()).or_default();
                d.calls += p.calls;
                d.tokens.add(&p.tokens);
            }
        }
    }
    s
}

impl Summary {
    pub fn capped_after_mean(&self) -> Option<i64> {
        (self.capped > 0).then(|| self.capped_after_sum / self.capped as i64)
    }

    fn mean(&self, v: usize) -> f64 {
        if self.windows == 0 {
            0.0
        } else {
            (v as f64 * 10.0 / self.windows as f64).round() / 10.0
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "windows": self.windows,
            "estimated_windows": self.estimated,
            "capped_windows": self.capped,
            "capped_after_minutes_mean": self.capped_after_mean().map(|s| s / 60),
            "calls": self.calls,
            "subagent_calls": self.subagent_calls,
            "calls_per_window": self.mean(self.calls),
            "parallel_sessions_max_mean": self.mean(self.parallel_max_sum),
            "tokens": self.tokens.to_json(),
            "tokens_per_point": per_point(&self.measured_tokens, self.points),
            "effort": parts_json(&self.by_effort, self.calls, self.tokens.output),
            "models": parts_json(&self.by_model, self.calls, self.tokens.output),
        })
    }
}

// --- Texte ---

/// `950`, `12,3 k`, `4,5 M`, `1,2 G`.
pub fn human(n: u64) -> String {
    let f = n as f64;
    let (v, unit) = if f >= 1e9 {
        (f / 1e9, " G")
    } else if f >= 1e6 {
        (f / 1e6, " M")
    } else if f >= 1e3 {
        (f / 1e3, " k")
    } else {
        return n.to_string();
    };
    format!("{v:.1}{unit}")
}

/// A decimal number, with a period.
pub fn decimal(v: f64) -> String {
    v.to_string()
}

pub fn tokens_line(t: &Tokens) -> String {
    format!(
        "input {}, cache read {}, cache written {}, output {} (reasoning {})",
        human(t.input),
        human(t.cache_read),
        human(t.cache_write),
        human(t.output),
        human(t.reasoning)
    )
}

/// `medium 80 %, high 20 %` (share of the calls), from the biggest to the smallest.
pub fn shares_line(map: &BTreeMap<String, Part>, calls: usize) -> String {
    let mut v: Vec<(&String, &Part)> = map.iter().collect();
    v.sort_by(|a, b| b.1.calls.cmp(&a.1.calls).then(a.0.cmp(b.0)));
    v.iter()
        .map(|(k, p)| {
            let k = k.as_str();
            format!("{k} {} %", decimal(pct(p.calls as u64, calls as u64)))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn render(st: &Stats) -> String {
    let w = &st.window;
    let mut head = format!(
        "{}  {} → {}",
        w.agent,
        time::short(w.start),
        time::local(w.end, "%H:%M")
    );
    if w.estimated {
        head.push_str("  (estimated, no reading)");
    }
    if let Some(p) = w.peak_percent {
        head.push_str(&format!("  pic {} %", decimal(p)));
    }
    if let Some(c) = w.capped_at {
        head.push_str(&format!(
            "  cap reached after {}",
            time::duration(c - w.start)
        ));
    }
    let mut out = format!(
        "{head}\n  {} calls ({} from sub-agents), {} sessions, {} in parallel at most\n",
        st.calls, st.subagent_calls, st.sessions, st.parallel_max
    );
    if st.calls > 0 {
        out.push_str(&format!("  tokens: {}\n", tokens_line(&st.tokens)));
        let pts = st.points();
        if pts > 0.0 {
            out.push_str(&format!(
                "  per quota point: output {}, total {}\n",
                human((st.tokens.output as f64 / pts) as u64),
                human((st.tokens.total() as f64 / pts) as u64)
            ));
        }
        out.push_str(&format!(
            "  effort: {}\n",
            shares_line(&st.by_effort, st.calls)
        ));
        out.push_str(&format!(
            "  models: {}\n",
            shares_line(&st.by_model, st.calls)
        ));
    }
    out
}

/// The same indicators side by side, one line per indicator.
pub fn render_compare(a: (&str, &Summary), b: (&str, &Summary)) -> String {
    let rows = |s: &Summary| -> Vec<String> {
        let pp = |v: u64| {
            if s.points > 0.0 {
                human((v as f64 / s.points) as u64)
            } else {
                "-".into()
            }
        };
        vec![
            format!("{} ({} estimated)", s.windows, s.estimated),
            match s.capped_after_mean() {
                Some(d) => format!("{} times, after {} on average", s.capped, time::duration(d)),
                None if s.windows == s.estimated => "unknown (no reading)".into(),
                None => "never".into(),
            },
            format!("{} ({} de sous-agents)", s.calls, s.subagent_calls),
            decimal(s.mean(s.calls)),
            decimal(s.mean(s.parallel_max_sum)),
            human(s.tokens.input),
            human(s.tokens.cache_read),
            human(s.tokens.cache_write),
            human(s.tokens.output),
            human(s.tokens.reasoning),
            format!(
                "sortie {}, total {}",
                pp(s.measured_tokens.output),
                pp(s.measured_tokens.total())
            ),
            shares_line(&s.by_effort, s.calls),
            shares_line(&s.by_model, s.calls),
        ]
    };
    let labels = [
        "5 h windows",
        "cap reached",
        "calls",
        "calls per window",
        "parallel sessions",
        "input",
        "cache read",
        "cache written",
        "output",
        "reasoning",
        "tokens per point",
        "effort (calls)",
        "models (calls)",
    ];
    let (ra, rb) = (rows(a.1), rows(b.1));
    let wa = ra
        .iter()
        .map(|s| s.chars().count())
        .max()
        .unwrap_or(0)
        .max(a.0.chars().count());
    let pad = |s: &str, w: usize| format!("{s}{}", " ".repeat(w.saturating_sub(s.chars().count())));
    let mut out = format!("{}  {}  {}\n", pad("", 22), pad(a.0, wa), b.0);
    for (i, l) in labels.iter().enumerate() {
        out.push_str(&format!("{}  {}  {}\n", pad(l, 22), pad(&ra[i], wa), rb[i]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::Limit;

    fn entry(t: i64, agent: &str, pct: f64, reset: i64) -> Entry {
        Entry {
            t: time::iso(t),
            agent: agent.into(),
            limits: vec![
                Limit {
                    label: "Session (5-hour)".into(),
                    percent: pct,
                    resets_at: time::iso(reset),
                },
                Limit {
                    label: "Weekly (7-day)".into(),
                    percent: 50.0,
                    resets_at: time::iso(reset + 86_400),
                },
            ],
        }
    }

    fn call(t: i64, session: &str, effort: Option<&str>, out: u64) -> Call {
        Call {
            t,
            agent: "claude",
            session: session.into(),
            project: "demo".into(),
            model: "model-a".into(),
            effort: effort.map(str::to_string),
            subagent: session == "sub",
            tokens: Tokens {
                input: 10,
                cache_read: 100,
                cache_write: 20,
                output: out,
                reasoning: out / 2,
            },
            key: format!("{session}-{t}"),
        }
    }

    const H: i64 = 3600;
    const T0: i64 = 1_700_000_000 - 1_700_000_000 % 3600; // on the hour sharp

    #[test]
    fn session_labels() {
        assert!(is_session_limit("Session (5-hour)"));
        assert!(is_session_limit("5h window"));
        assert!(!is_session_limit("Weekly (7-day)"));
        assert!(!is_session_limit("Fable Weekly"));
    }

    #[test]
    fn recorded_window_peak_and_cap() {
        let reset = T0 + 5 * H;
        let e = vec![
            entry(T0 + 600, "claude", 10.0, reset),
            entry(T0 + 2 * H, "claude", 100.0, reset + 40),
            entry(T0 + 3 * H, "claude", 100.0, reset),
        ];
        let w = history_windows(&e);
        assert_eq!(w.len(), 1);
        assert_eq!((w[0].start, w[0].end), (T0, reset));
        assert_eq!(w[0].peak_percent, Some(100.0));
        assert_eq!(w[0].capped_at, Some(T0 + 2 * H));
        let calls = vec![
            call(T0 + 60, "s1", Some("high"), 100),
            call(T0 + 120, "s2", Some("medium"), 300),
            call(T0 + 200, "sub", None, 100),
            call(T0 + 6 * H, "s1", Some("high"), 1),
        ];
        let st = stats(&w[0], &calls);
        assert_eq!(st.calls, 3);
        assert_eq!(st.subagent_calls, 1);
        assert_eq!(st.sessions, 3);
        assert_eq!(st.parallel_max, 3);
        assert_eq!(st.tokens.output, 500);
        let j = st.to_json();
        assert_eq!(j["capped_after_minutes"], 120);
        assert_eq!(j["effort"]["unknown"]["calls"], 1);
        assert_eq!(j["effort"]["medium"]["output_share"], 60.0);
        assert_eq!(j["tokens_per_point"]["output"], 5);
        assert_eq!(j["estimated"], false);
    }

    #[test]
    fn estimated_windows_before_the_history() {
        let reset = T0 + 20 * H;
        let e = vec![entry(T0 + 16 * H, "claude", 30.0, reset)];
        let calls = vec![
            call(T0 + 1800, "s1", None, 1),       // estimated window T0 → T0 + 5 h
            call(T0 + 4 * H, "s1", None, 1),      // same window
            call(T0 + 5 * H + 60, "s1", None, 1), // new one: T0 + 5 h
            call(T0 + 13 * H, "s1", None, 1), // T0 + 13 h, cut at the start of the recorded window (15 h)
            call(T0 + 16 * H, "s1", None, 1), // covered by the history
        ];
        let w = windows(&e, &calls, None, 0, i64::MAX);
        let got: Vec<(i64, i64, bool)> = w
            .iter()
            .map(|w| ((w.start - T0) / H, (w.end - T0) / H, w.estimated))
            .collect();
        assert_eq!(
            got,
            vec![(0, 5, true), (5, 10, true), (13, 15, true), (15, 20, false)]
        );
        let st = stats(&w[0], &calls);
        assert_eq!(st.calls, 2);
        assert_eq!(st.to_json()["tokens_per_point"], Value::Null);
    }

    #[test]
    fn summary_and_comparison() {
        let reset = T0 + 5 * H;
        let e = vec![entry(T0 + 3 * H, "claude", 100.0, reset)];
        let calls = vec![
            call(T0 + 60, "s1", Some("high"), 100),
            call(T0 + 6 * H, "s1", Some("low"), 50),
        ];
        let w = windows(&e, &calls, None, 0, i64::MAX);
        let all: Vec<Stats> = w.iter().map(|w| stats(w, &calls)).collect();
        let s = summarize(&all);
        assert_eq!((s.windows, s.estimated, s.capped, s.calls), (2, 1, 1, 2));
        assert_eq!(s.capped_after_mean(), Some(3 * H));
        let j = s.to_json();
        assert_eq!(j["calls_per_window"], 1.0);
        // Only the recorded window counts for the tokens per point: 100 of output / 100 points.
        assert_eq!(j["tokens_per_point"]["output"], 1);
        let text = render_compare(("A", &s), ("B", &Summary::default()));
        assert!(text.contains("cap reached"));
        assert!(text.contains("unknown (no reading)"));
    }

    #[test]
    fn readable_numbers() {
        assert_eq!(human(950), "950");
        assert_eq!(human(12_345), "12.3 k");
        assert_eq!(human(4_500_000), "4.5 M");
    }
}
