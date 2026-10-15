//! The binary end to end, on invented logs and an invented collector (temporary folder:
//! never the real logs, never the real history).

use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Fx {
    _d: tempfile::TempDir,
    home: PathBuf,
    bin: PathBuf,
}

fn write(p: &Path, s: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, s).unwrap();
}

fn jsonl(lines: &[Value]) -> String {
    lines.iter().map(|l| format!("{l}\n")).collect()
}

fn assistant(t: &str, req: &str, session: &str, out: u64, extra: Value) -> Value {
    let mut v = json!({
        "type": "assistant",
        "timestamp": t,
        "requestId": req,
        "sessionId": session,
        "cwd": "/home/demo/projects/example",
        "isSidechain": false,
        "effort": "medium",
        "perTurnEffort": "high",
        "message": {
            "id": format!("msg_{req}"),
            "model": "claude-test-1",
            "content": [{"type": "text", "text": "invented content"}],
            "usage": {
                "input_tokens": 10,
                "cache_creation_input_tokens": 200,
                "cache_read_input_tokens": 3000,
                "output_tokens": out,
                "output_tokens_details": {"thinking_tokens": out / 4}
            }
        }
    });
    if let Value::Object(m) = extra {
        for (k, x) in m {
            v[k] = x;
        }
    }
    v
}

fn fx() -> Fx {
    let d = tempfile::tempdir().unwrap();
    let home = d.path().join("home");
    let proj = home.join(".claude/projects/-home-demo-projects-example");
    // Main session: the same call on three lines (one incomplete), a synthetic message,
    // a call with no effort noted.
    write(
        &proj.join("s1.jsonl"),
        &jsonl(&[
            json!({"type": "user", "timestamp": "2026-01-10T10:00:00Z", "message": {"content": "assistant"}}),
            assistant("2026-01-10T10:00:05Z", "req_a", "s1", 0, json!({})),
            assistant("2026-01-10T10:00:06Z", "req_a", "s1", 400, json!({})),
            assistant("2026-01-10T10:00:06Z", "req_a", "s1", 400, json!({})),
            assistant(
                "2026-01-10T10:01:00Z",
                "req_b",
                "s1",
                100,
                json!({"effort": null, "perTurnEffort": null}),
            ),
            {
                let mut v = assistant("2026-01-10T10:02:00Z", "req_x", "s1", 0, json!({}));
                v["message"]["model"] = "<synthetic>".into();
                v
            },
        ]),
    );
    // Resumed session: copies req_b, then a new call.
    write(
        &proj.join("s2.jsonl"),
        &jsonl(&[
            assistant(
                "2026-01-10T10:01:00Z",
                "req_b",
                "s1",
                100,
                json!({"effort": null, "perTurnEffort": null}),
            ),
            assistant(
                "2026-01-10T12:00:00Z",
                "req_c",
                "s2",
                50,
                json!({"perTurnEffort": "low"}),
            ),
        ]),
    );
    // Sub-agent, in a sub-folder.
    write(
        &proj.join("s1/subagents/agent-a1.jsonl"),
        &jsonl(&[assistant(
            "2026-01-10T10:30:00Z",
            "req_s",
            "s1",
            70,
            json!({"isSidechain": true}),
        )]),
    );
    // Recent Codex: records per response (and token_count, ignored).
    let codex = home.join(".codex/sessions/2026/01/10");
    write(
        &codex.join("rollout-new.jsonl"),
        &jsonl(&[
            json!({"timestamp": "2026-01-10T11:00:00Z", "type": "session_meta", "payload": {"id": "c1", "cwd": "/home/demo/projects/other", "source": "cli"}}),
            json!({"timestamp": "2026-01-10T11:00:01Z", "type": "turn_context", "payload": {"turn_id": "t1", "model": "gpt-test", "effort": "high"}}),
            json!({"timestamp": "2026-01-10T11:00:05Z", "type": "token_usage_record", "payload": {"response_id": "resp_1", "turn_id": "t1", "usage": {"input_tokens": 1000, "cached_input_tokens": 800, "cache_write_input_tokens": 0, "output_tokens": 60, "reasoning_output_tokens": 20, "total_tokens": 1060}}}),
            json!({"timestamp": "2026-01-10T11:00:05Z", "type": "event_msg", "payload": {"type": "token_count", "info": {"total_token_usage": {"total_tokens": 1060}, "last_token_usage": {"input_tokens": 1000, "output_tokens": 60, "total_tokens": 1060}}}}),
        ]),
    );
    // Old Codex, sub-agent: only token_count, one of which is repeated.
    let tc = |t: &str, total: u64, out: u64| {
        json!({"timestamp": t, "type": "event_msg", "payload": {"type": "token_count", "info": {
            "total_token_usage": {"input_tokens": total, "output_tokens": 0, "total_tokens": total},
            "last_token_usage": {"input_tokens": 500, "cached_input_tokens": 100, "output_tokens": out, "reasoning_output_tokens": 0, "total_tokens": 500 + out}}}})
    };
    write(
        &codex.join("rollout-old.jsonl"),
        &jsonl(&[
            json!({"timestamp": "2026-01-10T11:10:00Z", "type": "session_meta", "payload": {"id": "c2", "cwd": "/home/demo/projects/other", "source": {"subagent": {"other": "guardian"}}}}),
            json!({"timestamp": "2026-01-10T11:10:01Z", "type": "turn_context", "payload": {"model": "gpt-test", "effort": "low"}}),
            tc("2026-01-10T11:10:05Z", 510, 10),
            tc("2026-01-10T11:10:06Z", 510, 10),
            tc("2026-01-10T11:11:00Z", 1030, 20),
        ]),
    );
    // Omarchy's collector (fresh: no reading redone) and fake omarchy-agent-usage-update.
    let usage = home.join(".local/state/omarchy/agents/usage");
    write(
        &usage.join("claude.json"),
        &json!({"id": "claude", "name": "Claude", "usageStatusText": "", "limits": [
            {"label": "Session (5-hour)", "percent": 0.42, "resetsAt": "2026-01-10T14:59:59.5+00:00"},
            {"label": "Weekly (7-day)", "percent": 0.1, "resetsAt": "2026-01-15T12:00:00+00:00"}
        ]})
        .to_string(),
    );
    write(
        &usage.join("broken.json"),
        &json!({"id": "broken", "name": "Broken", "usageStatusText": "Login expired", "authHelpText": "Log in again.", "limits": []}).to_string(),
    );
    let bin = d.path().join("bin");
    write(
        &bin.join("omarchy-agent-usage-update"),
        &format!(
            "#!/bin/sh\necho \"$OMARCHY_PATH $*\" >> '{}'\n",
            d.path().join("calls.log").display()
        ),
    );
    fs::set_permissions(
        bin.join("omarchy-agent-usage-update"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    Fx { _d: d, home, bin }
}

fn run(f: &Fx, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_poste-usage"))
        .args(args)
        .env_clear()
        .env("HOME", &f.home)
        .env("PATH", format!("{}:/usr/bin:/bin", f.bin.display()))
        .env("TZ", "UTC")
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn json_lines(s: &str) -> Vec<Value> {
    s.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

const DAY: [&str; 4] = [
    "--since",
    "2026-01-10T00:00:00Z",
    "--until",
    "2026-01-11T00:00:00Z",
];

#[test]
fn deduplicated_calls_sub_agents_and_effort() {
    let f = fx();
    let (code, out, err) = run(&f, &[&["calls", "--json"][..], &DAY].concat());
    assert_eq!(code, 0, "{err}");
    let calls = json_lines(&out);
    let claude: Vec<&Value> = calls.iter().filter(|c| c["agent"] == "claude").collect();
    // req_a (once, the complete version), req_b (once despite the resume), req_c, req_s.
    assert_eq!(claude.len(), 4, "{out}");
    let a = &claude[0];
    assert_eq!(a["output"], 400);
    assert_eq!(a["reasoning"], 100);
    assert_eq!(a["cache_read"], 3000);
    assert_eq!(a["cache_write"], 200);
    assert_eq!(a["input"], 10);
    assert_eq!(a["effort"], "high", "perTurnEffort first");
    assert_eq!(a["project"], "example");
    assert_eq!(a["model"], "claude-test-1");
    assert_eq!(
        claude[1]["effort"],
        Value::Null,
        "absent effort: null, never guessed"
    );
    assert_eq!(claude[2]["subagent"], true);
    assert_eq!(claude[2]["time"], "2026-01-10T10:30:00Z");
    assert_eq!(claude[3]["effort"], "low");

    let codex: Vec<&Value> = calls.iter().filter(|c| c["agent"] == "codex").collect();
    assert_eq!(codex.len(), 3, "{out}");
    assert_eq!(codex[0]["input"], 200, "input outside the cache");
    assert_eq!(codex[0]["cache_read"], 800);
    assert_eq!(codex[0]["reasoning"], 20);
    assert_eq!(codex[0]["effort"], "high");
    assert_eq!(codex[0]["subagent"], false);
    assert_eq!(codex[1]["subagent"], true);
    assert_eq!(codex[1]["output"], 10);
    assert_eq!(codex[2]["output"], 20);
    assert_eq!(codex[2]["effort"], "low");

    let (_, only, _) = run(
        &f,
        &[&["calls", "--json", "--agent", "codex"][..], &DAY].concat(),
    );
    assert_eq!(json_lines(&only).len(), 3);
    let (_, text, _) = run(&f, &[&["calls"][..], &DAY].concat());
    assert_eq!(text.lines().count(), 7);
    assert!(!text.contains("invented content"));
}

#[test]
fn reading_history_windows_and_comparison() {
    let f = fx();
    let (code, _, err) = run(&f, &["record"]);
    assert_eq!(code, 0, "{err}");
    run(&f, &["record"]);
    let hist = f.home.join(".local/state/poste/usage/history.jsonl");
    let lines = fs::read_to_string(&hist).unwrap();
    assert_eq!(
        lines.lines().count(),
        1,
        "nothing new at the second reading: {lines}"
    );
    assert!(lines.contains("\"resetsAt\":\"2026-01-10T15:00:00Z\""));
    assert!(!lines.contains("broken"));
    let log = fs::read_to_string(f.home.parent().unwrap().join("calls.log")).unwrap();
    assert!(log.contains("--limits-only"));
    assert!(
        log.starts_with(&f.bin.parent().unwrap().display().to_string()),
        "OMARCHY_PATH deduced"
    );

    // History: the time of the reading is now; wide --since.
    let (code, out, _) = run(&f, &["history", "--since", "2020-01-01", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(json_lines(&out)[0]["limits"][0]["percent"], 42);
    let (_, out, _) = run(&f, &["history", "codex", "--since", "2020-01-01"]);
    assert!(out.is_empty());

    // Window recorded 10:00 → 15:00 (reset at 15:00), Claude calls from 10:00 to 12:00.
    let (code, out, err) = run(
        &f,
        &[&["windows", "--json", "--agent", "claude"][..], &DAY].concat(),
    );
    assert_eq!(code, 0, "{err}");
    let w = json_lines(&out);
    assert_eq!(w.len(), 1, "{out}");
    assert_eq!(w[0]["start"], "2026-01-10T10:00:00Z");
    assert_eq!(w[0]["estimated"], false);
    assert_eq!(w[0]["peak_percent"], 42.0);
    assert_eq!(w[0]["calls"], 4);
    assert_eq!(w[0]["subagent_calls"], 1);
    assert_eq!(w[0]["sessions"], 2);
    assert_eq!(w[0]["tokens"]["output"], 620);
    assert_eq!(w[0]["effort"]["high"]["calls"], 2);
    assert_eq!(w[0]["effort"]["unknown"]["calls"], 1);

    // Codex: no reading, window estimated from the first call.
    let (_, out, _) = run(
        &f,
        &[&["windows", "--json", "--agent", "codex"][..], &DAY].concat(),
    );
    let w = json_lines(&out);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0]["estimated"], true);
    assert_eq!(w[0]["start"], "2026-01-10T11:00:05Z");
    assert_eq!(w[0]["tokens_per_point"], Value::Null);

    let (code, out, err) = run(
        &f,
        &[
            "compare",
            "2026-01-09T00:00:00Z..2026-01-10T00:00:00Z",
            "2026-01-10T00:00:00Z..2026-01-11T00:00:00Z",
            "--json",
        ],
    );
    assert_eq!(code, 0, "{err}");
    let c: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(c["a"]["windows"], 0);
    assert_eq!(c["b"]["windows"], 2);
    assert_eq!(c["b"]["estimated_windows"], 1);
    assert_eq!(c["b"]["calls"], 7);
    assert_eq!(c["b"]["since"], "2026-01-10T00:00:00Z");
    let (_, text, _) = run(
        &f,
        &[
            "compare",
            "2026-01-09T00:00:00Z..2026-01-10T00:00:00Z",
            "2026-01-10T00:00:00Z..2026-01-11T00:00:00Z",
        ],
    );
    assert!(text.contains("5 h windows"));
    let (code, _, err) = run(&f, &["compare", "yesterday"]);
    assert_eq!(code, 2);
    assert!(err.contains("two periods"));
}

#[test]
fn quotas_of_the_collector() {
    let f = fx();
    let (code, out, _) = run(&f, &[]);
    assert_eq!(code, 0);
    assert!(out.contains("Claude (reading 0 min ago)"));
    assert!(
        out.contains("Session (5-hour)      42 %, resets on 10/01 at 15:00"),
        "{out}"
    );
    let (code, out, _) = run(&f, &["--all"]);
    assert_eq!(code, 1, "an agent in error");
    assert!(out.contains("Login expired") && out.contains("Log in again."));
    let (code, _, err) = run(&f, &["unknown"]);
    assert_eq!(code, 1);
    assert!(err.contains("no reading"));
    let (code, _, _) = run(&f, &["--nope"]);
    assert_eq!(code, 2);
    // Fresh reading: the collector was not run again for claude.
    assert!(
        !f.home.parent().unwrap().join("calls.log").exists() || {
            let log = fs::read_to_string(f.home.parent().unwrap().join("calls.log")).unwrap();
            !log.contains("claude")
        }
    );
}
