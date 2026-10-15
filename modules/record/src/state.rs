//! `state.json` (recording in progress) and its public form (`status`, `watch`).

use crate::env::Env;
use crate::proc::now_ms;
use crate::pyval::{dumps, truthy};
use serde::Serialize;
use serde_json::Value;

/// Content of `state.json`, in the writing order of the original script.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct State {
    /// pid de gpu-screen-recorder.
    pub pid: i64,
    /// Start, in milliseconds since the epoch.
    pub started: f64,
    #[serde(rename = "pausedAt")]
    pub paused_at: Option<f64>,
    #[serde(rename = "pausedTotal")]
    pub paused_total: f64,
    pub file: String,
    pub label: String,
}

/// Public state: `{"t": "status", "recording": false}` at rest.
#[derive(Debug, Serialize, PartialEq)]
pub struct Status {
    pub t: &'static str,
    pub recording: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paused: Option<bool>,
    #[serde(rename = "recordMs", skip_serializing_if = "Option::is_none")]
    pub record_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
}

fn num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(*b as u8 as f64),
        _ => None,
    }
}

/// `int(st.get("pid"))`: number or numeric text.
fn pid_of(v: Option<&Value>) -> Option<i64> {
    match v? {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

/// Does the pid exist (`os.kill(pid, 0)`)?
fn alive(pid: i64) -> bool {
    // A zero or negative pid would target a process group: never a recorder (deliberate difference).
    pid > 0 && unsafe { libc::kill(pid as libc::pid_t, 0) } == 0
}

/// `read_state()`: the recording in progress, if `state.json` exists and its process is alive.
pub fn read_state(env: &Env) -> Option<State> {
    let text = std::fs::read_to_string(env.state_file()).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let o = v.as_object()?;
    let pid = pid_of(o.get("pid"))?;
    if !alive(pid) {
        return None;
    }
    let text_of = |k: &str| match o.get(k) {
        Some(Value::String(s)) => s.clone(),
        Some(other) => crate::pyval::py_str(other),
        None => String::new(),
    };
    Some(State {
        pid,
        started: num(o.get("started")).unwrap_or(0.0),
        paused_at: o
            .get("pausedAt")
            .filter(|p| truthy(p))
            .and_then(|p| num(Some(p))),
        paused_total: num(o.get("pausedTotal")).unwrap_or(0.0),
        file: text_of("file"),
        label: text_of("label"),
    })
}

/// `write_state(st)`: write to a temporary file then rename.
pub fn write_state(env: &Env, st: &State) -> std::io::Result<()> {
    std::fs::create_dir_all(&env.run)?;
    let file = env.state_file();
    let tmp = env.run.join("state.json.tmp");
    std::fs::write(&tmp, dumps(st))?;
    std::fs::rename(tmp, file)
}

/// `public(st)` with the time `now` (milliseconds) injected for the tests.
pub fn public_at(st: Option<&State>, now: f64) -> Status {
    let Some(st) = st else {
        return Status {
            t: "status",
            recording: false,
            paused: None,
            record_ms: None,
            label: None,
            file: None,
        };
    };
    let elapsed = st.paused_at.unwrap_or(now) - st.started - st.paused_total;
    Status {
        t: "status",
        recording: true,
        paused: Some(st.paused_at.is_some()),
        record_ms: Some(elapsed.max(0.0) as i64),
        label: Some(st.label.clone()),
        file: Some(st.file.clone()),
    }
}

pub fn public(st: Option<&State>) -> Status {
    public_at(st, now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(paused_at: Option<f64>) -> State {
        State {
            pid: 1,
            started: 1000.0,
            paused_at,
            paused_total: 500.0,
            file: "/v/a.mp4".into(),
            label: "Unity".into(),
        }
    }

    #[test]
    fn repos() {
        assert_eq!(
            dumps(&public_at(None, 0.0)),
            r#"{"t": "status", "recording": false}"#
        );
    }

    #[test]
    fn en_cours_et_en_pause() {
        assert_eq!(
            dumps(&public_at(Some(&st(None)), 11_000.0)),
            r#"{"t": "status", "recording": true, "paused": false, "recordMs": 9500, "label": "Unity", "file": "/v/a.mp4"}"#
        );
        // Paused, the time is frozen at the instant of the pause.
        assert_eq!(
            public_at(Some(&st(Some(3000.0))), 99_999.0).record_ms,
            Some(1500)
        );
        // Never negative.
        assert_eq!(public_at(Some(&st(Some(1200.0))), 0.0).record_ms, Some(0));
    }
}
