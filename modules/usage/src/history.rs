//! History of the readings: `~/.local/state/poste/usage/history.jsonl`, one line per agent and per
//! reading, only when its limits changed since its previous line. A single writer:
//! `poste usage record` (the timer `poste-usage-record.timer`, every 5 minutes).
//!
//! A line: `{"t", "agent", "limits": [{"label", "percent", "resetsAt"}]}`. `t`: time of the
//! run (UTC); `percent`: 0 to 100, one decimal; `resetsAt`: reset (UTC) rounded to the
//! minute (the server gives it to the microsecond and it moves from one reading to the next). No token,
//! no account identifier.

use crate::time;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    pub label: String,
    #[serde(serialize_with = "compact_number")]
    pub percent: f64,
    #[serde(rename = "resetsAt")]
    pub resets_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub t: String,
    pub agent: String,
    pub limits: Vec<Limit>,
}

/// `11` rather than `11.0`: like the lines written by jq.
fn compact_number<S: serde::Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        s.serialize_i64(*v as i64)
    } else {
        s.serialize_f64(*v)
    }
}

/// Reset rounded to the minute; unreadable: kept as is.
fn round_reset(s: &str) -> String {
    match time::parse_iso(s) {
        Some(t) => time::iso((t + 30).div_euclid(60) * 60),
        None => s.to_string(),
    }
}

/// The collector's reading reduced to the history; nothing for an agent in error or without a limit.
pub fn snapshot(agent: &str, record: &Value, now: i64) -> Option<Entry> {
    let status = record.get("usageStatusText").and_then(Value::as_str);
    if status.is_some_and(|s| !s.is_empty()) {
        return None;
    }
    let limits: Vec<Limit> = record
        .get("limits")?
        .as_array()?
        .iter()
        .map(|l| {
            let s = |k: &str| l.get(k).and_then(Value::as_str).unwrap_or("");
            let label = if s("title").is_empty() {
                s("label")
            } else {
                s("title")
            };
            let pct = l.get("percent").and_then(Value::as_f64).unwrap_or(0.0);
            Limit {
                label: label.to_string(),
                percent: (pct * 1000.0).round() / 10.0,
                resets_at: round_reset(s("resetsAt")),
            }
        })
        .collect();
    (!limits.is_empty()).then(|| Entry {
        t: time::iso(now),
        agent: agent.to_string(),
        limits,
    })
}

pub fn load(file: &Path) -> Vec<Entry> {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Adds the readings whose limits changed; returns the number of lines written.
pub fn append_changed(file: &Path, snaps: &[Entry]) -> Result<usize> {
    let mut last: HashMap<String, Vec<Limit>> = HashMap::new();
    for e in load(file) {
        last.insert(e.agent, e.limits);
    }
    let fresh: Vec<&Entry> = snaps
        .iter()
        .filter(|s| last.get(&s.agent) != Some(&s.limits))
        .collect();
    if fresh.is_empty() {
        return Ok(0);
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
        .with_context(|| format!("ouverture de {}", file.display()))?;
    let mut buf = String::new();
    for e in &fresh {
        buf.push_str(&serde_json::to_string(e)?);
        buf.push('\n');
    }
    f.write_all(buf.as_bytes())?;
    Ok(fresh.len())
}

/// A readable line: local time, agent, each limit and its reset.
pub fn render(e: &Entry) -> String {
    let t = time::parse_iso(&e.t)
        .map(time::short)
        .unwrap_or_else(|| e.t.clone());
    let limits: Vec<String> = e
        .limits
        .iter()
        .map(|l| {
            let reset = time::parse_iso(&l.resets_at)
                .map(|r| format!(" ({})", time::short(r)))
                .unwrap_or_default();
            format!("{} {} %{reset}", l.label, crate::stats::decimal(l.percent))
        })
        .collect();
    format!("{t}  {}  {}", e.agent, limits.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(pct: f64, reset: &str) -> Value {
        json!({"id": "claude", "usageStatusText": "", "limits": [
            {"label": "Session (5-hour)", "percent": pct, "resetsAt": reset},
            {"label": "Fable Weekly", "title": "Fable Weekly", "percent": 0.0, "resetsAt": ""}
        ]})
    }

    #[test]
    fn snapshot_rounded_and_without_error() {
        let e = snapshot(
            "claude",
            &record(0.11, "2026-10-14T20:59:59.588385+00:00"),
            0,
        )
        .unwrap();
        assert_eq!(e.limits[0].percent, 11.0);
        assert_eq!(e.limits[0].resets_at, "2026-10-14T21:00:00Z");
        assert_eq!(e.limits[1].label, "Fable Weekly");
        assert_eq!(e.limits[1].resets_at, "");
        let err = json!({"usageStatusText": "Login expired", "limits": []});
        assert!(snapshot("claude", &err, 0).is_none());
        assert!(snapshot("x", &json!({"limits": []}), 0).is_none());
    }

    #[test]
    fn one_line_only_when_it_changes() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("usage/history.jsonl");
        let a = snapshot("claude", &record(0.11, "2026-10-14T20:59:59.58+00:00"), 0).unwrap();
        assert_eq!(append_changed(&f, std::slice::from_ref(&a)).unwrap(), 1);
        // Same reading, reset that moves by a few microseconds: nothing.
        let b = snapshot("claude", &record(0.11, "2026-10-14T20:59:59.91+00:00"), 300).unwrap();
        assert_eq!(append_changed(&f, &[b]).unwrap(), 0);
        let c = snapshot("claude", &record(0.13, "2026-10-14T20:59:59.91+00:00"), 600).unwrap();
        let x = snapshot("codex", &record(0.5, "2026-10-14T18:11:30+00:00"), 600).unwrap();
        assert_eq!(append_changed(&f, &[c, x]).unwrap(), 2);
        let all = load(&f);
        assert_eq!(all.len(), 3);
        assert_eq!(all[2].agent, "codex");
        let text = std::fs::read_to_string(&f).unwrap();
        assert!(text.starts_with(
            "{\"t\":\"1970-01-01T00:00:00Z\",\"agent\":\"claude\",\"limits\":[{\"label\":\"Session (5-hour)\",\"percent\":11,"
        ));
    }
}
