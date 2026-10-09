//! Notification discrète (urgence basse) au style de l'île : `omarchy-notification-send`.
//! `POSTE_DISPLAY_NOTIFY` remplace le programme (RÉSERVÉE AUX TESTS). Échec sans conséquence.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const NOTIFY_VAR: &str = "POSTE_DISPLAY_NOTIFY";
const PROGRAM: &str = "omarchy-notification-send";
const GLYPH: &str = "\u{f0379}";

fn program() -> PathBuf {
    if let Some(v) = std::env::var_os(NOTIFY_VAR).filter(|v| !v.is_empty()) {
        return v.into();
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    dirs.push(PathBuf::from("/usr/share/omarchy/bin"));
    if let Some(h) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(h).join(".local/share/omarchy/bin"));
    }
    dirs.into_iter()
        .map(|d| d.join(PROGRAM))
        .find(|p| p.is_file())
        .unwrap_or_else(|| PROGRAM.into())
}

pub fn send(title: &str, body: &str) -> bool {
    let Ok(mut child) = Command::new(program())
        .args([
            "--app-name",
            "Écrans",
            "-g",
            GLYPH,
            "-u",
            "low",
            title,
            body,
        ])
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
