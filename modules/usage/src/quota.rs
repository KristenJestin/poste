//! Quotas lus dans le collecteur d'Omarchy (`omarchy-agent-usage-update`,
//! `~/.local/state/omarchy/agents/usage/<agent>.json`), la même source que le centre de contrôle de
//! l'île. Lecture seule : c'est le collecteur qui écrit ces fichiers.

use crate::time;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Âge au-delà duquel un relevé est refait avant d'être affiché.
pub const MAX_AGE: i64 = 300;

pub fn collector_dir(state_home: &Path) -> PathBuf {
    state_home.join("omarchy/agents/usage")
}

/// Les agents relevés par le collecteur (un fichier chacun), triés.
pub fn known_agents(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.strip_suffix(".json").map(str::to_string)
        })
        .filter(|n| !n.starts_with('.'))
        .collect();
    out.sort();
    out
}

/// Âge du relevé en secondes (None : pas de relevé).
pub fn age(file: &Path) -> Option<i64> {
    let m = std::fs::metadata(file).ok()?.modified().ok()?;
    let secs = m.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64;
    Some(time::now() - secs)
}

/// Lance le collecteur d'Omarchy (`--limits-only`) pour ces agents (tous si vide). Hors session
/// (minuteur), `OMARCHY_PATH` peut manquer : il sert au collecteur à trouver les collecteurs de
/// chaque agent, on le déduit du chemin de `omarchy-agent-usage-update`.
pub fn refresh(path: &[PathBuf], agents: &[String]) {
    let Some(bin) = path
        .iter()
        .map(|d| d.join("omarchy-agent-usage-update"))
        .find(|p| p.is_file())
    else {
        eprintln!("poste usage : omarchy-agent-usage-update introuvable, relevé non actualisé");
        return;
    };
    let mut cmd = Command::new(&bin);
    if std::env::var_os("OMARCHY_PATH").is_none_or(|v| v.is_empty()) {
        if let Some(root) = bin.parent().and_then(Path::parent) {
            cmd.env("OMARCHY_PATH", root);
        }
    }
    let _ = cmd
        .arg("--limits-only")
        .args(agents)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub fn read(file: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

fn str_of<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Le texte affiché pour un agent ; faux si le relevé signale une erreur.
pub fn render(v: &Value, age_secs: i64) -> (String, bool) {
    let name = [str_of(v, "name"), str_of(v, "title"), str_of(v, "id")]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or("?");
    let mut out = format!("{name} (relevé il y a {} min)\n", age_secs / 60);
    let status = str_of(v, "usageStatusText");
    if !status.is_empty() {
        out.push_str(&format!("  {status}\n"));
        let hint = str_of(v, "authHelpText");
        if !hint.is_empty() {
            out.push_str(&format!("  {hint}\n"));
        }
        return (out, false);
    }
    for l in v
        .get("limits")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let label = [str_of(l, "title"), str_of(l, "label")]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or("?");
        let pct = (l.get("percent").and_then(Value::as_f64).unwrap_or(0.0) * 100.0).round();
        // +1 s : une remise annoncée à 17:59:59 s'affiche 18:00.
        let when = time::parse_iso(str_of(l, "resetsAt"))
            .map(|t| format!(", remise à zéro le {}", time::local(t + 1, "%d/%m à %H:%M")))
            .unwrap_or_default();
        out.push_str(&format!("  {label:<20} {pct:>3} %{when}\n"));
    }
    (out, true)
}
