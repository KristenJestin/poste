//! Dialogue avec Hyprland : `hyprctl` (remplaçable par `POSTE_DISPLAY_HYPRCTL`, RÉSERVÉE AUX TESTS),
//! socket d'événements `.socket2.sock`, instance courante.

use crate::plan::Cmd;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const HYPRCTL_VAR: &str = "POSTE_DISPLAY_HYPRCTL";

fn program() -> OsString {
    std::env::var_os(HYPRCTL_VAR)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "hyprctl".into())
}

/// Signature de l'instance Hyprland : la variable d'environnement, sinon le dossier le plus récent
/// de `$XDG_RUNTIME_DIR/hypr/` qui a un socket d'événements (le démon systemd n'a pas toujours
/// la variable).
pub fn instance_signature() -> Option<String> {
    if let Some(s) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .ok()
        .filter(|s| !s.is_empty())
    {
        return Some(s);
    }
    let base = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?).join("hypr");
    newest_instance(&base)
}

pub fn newest_instance(base: &Path) -> Option<String> {
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for e in std::fs::read_dir(base).ok()?.flatten() {
        let sock = e.path().join(".socket2.sock");
        let Ok(md) = std::fs::metadata(&sock) else {
            continue;
        };
        let t = md.modified().unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(bt, _)| t > *bt) {
            best = Some((t, e.file_name().to_string_lossy().into_owned()));
        }
    }
    best.map(|(_, n)| n)
}

pub fn event_socket() -> Option<PathBuf> {
    let sig = instance_signature()?;
    Some(
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?)
            .join("hypr")
            .join(sig)
            .join(".socket2.sock"),
    )
}

/// Sortie standard de `hyprctl <args>`, 5 s au plus ; `None` si échec.
pub fn hyprctl(args: &[&str]) -> Option<String> {
    let mut c = Command::new(program());
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(sig) = instance_signature() {
        c.env("HYPRLAND_INSTANCE_SIGNATURE", sig);
    }
    let mut child = c.spawn().ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut out, &mut s);
        s
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let s = reader.join().ok()?;
                return st.success().then_some(s);
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

pub fn monitors_json() -> Option<String> {
    hyprctl(&["monitors", "all", "-j"])
}

pub fn clients_json() -> String {
    hyprctl(&["clients", "-j"]).unwrap_or_else(|| "[]".into())
}

fn is_ok(out: &Option<String>) -> bool {
    out.as_deref().is_some_and(|s| s.trim() == "ok")
}

/// Exécute une commande : forme Lua d'abord, forme historique si elle ne répond pas « ok ».
/// Renvoie `Err(message)` si aucune ne marche.
pub fn run(cmd: &Cmd) -> Result<(), String> {
    if is_ok(&hyprctl(&["eval", &cmd.lua])) {
        return Ok(());
    }
    if let Some(args) = &cmd.legacy {
        let a: Vec<&str> = args.iter().map(String::as_str).collect();
        if is_ok(&hyprctl(&a)) {
            return Ok(());
        }
    }
    Err(format!("{} : Hyprland a refusé la commande", cmd.desc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_la_plus_recente() {
        let t = tempfile::tempdir().unwrap();
        assert!(newest_instance(t.path()).is_none());
        std::fs::create_dir_all(t.path().join("vide")).unwrap();
        assert!(newest_instance(t.path()).is_none());
        std::fs::create_dir_all(t.path().join("abc")).unwrap();
        std::fs::write(t.path().join("abc/.socket2.sock"), "").unwrap();
        assert_eq!(newest_instance(t.path()).as_deref(), Some("abc"));
    }
}
