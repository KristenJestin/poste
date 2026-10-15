//! Notifications in the island's style: `omarchy-notification-send`, 5 s at most, a failure is
//! harmless (returned so that the alert is retried). `POSTE_POWER_NOTIFY` replaces the program (TESTS).

use crate::engine::Alert;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const NOTIFY_VAR: &str = "POSTE_POWER_NOTIFY";
const PROGRAM: &str = "omarchy-notification-send";

/// Where to look for the program when the daemon's PATH is short.
fn program() -> PathBuf {
    if let Some(v) = std::env::var_os(NOTIFY_VAR).filter(|v| !v.is_empty()) {
        return v.into();
    }
    if let Some(path) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&path) {
            let p = d.join(PROGRAM);
            if p.is_file() {
                return p;
            }
        }
    }
    let mut extra = vec![PathBuf::from("/usr/share/omarchy/bin")];
    if let Some(h) = std::env::var_os("HOME") {
        extra.push(PathBuf::from(h).join(".local/share/omarchy/bin"));
    }
    extra
        .into_iter()
        .map(|d| d.join(PROGRAM))
        .find(|p| p.is_file())
        .unwrap_or_else(|| PROGRAM.into())
}

/// The arguments, kept apart from the sending so that they can be tested.
pub fn args(a: &Alert) -> Vec<String> {
    vec![
        "--app-name".into(),
        "Power".into(),
        "-g".into(),
        a.glyph.into(),
        "-u".into(),
        a.urgency.as_str().into(),
        a.title.clone(),
        a.body.clone(),
    ]
}

pub fn send(a: &Alert) -> bool {
    let Ok(mut child) = Command::new(program())
        .args(args(a))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(s)) => return s.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}
