//! Chemins et configuration (préréglages), comme les constantes du script d'origine.

use crate::pyval::py_str;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

pub struct Env {
    /// `os.path.expanduser("~")`.
    pub home: String,
    /// `$XDG_RUNTIME_DIR/poste-record` (état, pick.json, last.log).
    pub run: PathBuf,
    pub config: PathBuf,
    /// `$XDG_VIDEOS_DIR` ou `~/Videos`.
    pub videos: String,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn nonempty(name: &str) -> Option<String> {
    var(name).filter(|v| !v.is_empty())
}

impl Env {
    pub fn from_env() -> Env {
        let home = match var("HOME") {
            Some(h) => {
                let t = h.trim_end_matches('/');
                if t.is_empty() {
                    "/".to_string()
                } else {
                    t.to_string()
                }
            }
            None => "/".to_string(),
        };
        let runtime = var("XDG_RUNTIME_DIR").unwrap_or_else(|| "/tmp".into());
        let config_home = nonempty("XDG_CONFIG_HOME").unwrap_or_else(|| format!("{home}/.config"));
        let videos = nonempty("XDG_VIDEOS_DIR").unwrap_or_else(|| format!("{home}/Videos"));
        Env {
            run: PathBuf::from(runtime).join("poste-record"),
            config: PathBuf::from(config_home).join("poste/record/presets.json"),
            videos,
            home,
        }
    }

    pub fn state_file(&self) -> PathBuf {
        self.run.join("state.json")
    }
    pub fn pick_file(&self) -> PathBuf {
        self.run.join("pick.json")
    }
    pub fn log_file(&self) -> PathBuf {
        self.run.join("last.log")
    }

    /// `os.path.expanduser` (forme `~` et `~/…` seulement).
    pub fn expanduser(&self, p: &str) -> String {
        if !p.starts_with('~') {
            return p.to_string();
        }
        let i = p.find('/').unwrap_or(p.len());
        if i != 1 {
            return p.to_string();
        }
        let r = format!("{}{}", self.home, &p[i..]);
        if r.is_empty() {
            "/".into()
        } else {
            r
        }
    }
}

/// Valeurs par défaut de `defaults` (DEFAULT_CONFIG du script).
fn default_defaults() -> Map<String, Value> {
    let v = json!({"dir": "~/Videos", "fps": 60, "quality": "very_high", "codec": "auto", "container": "mp4", "cursor": true});
    v.as_object().cloned().unwrap_or_default()
}

/// `load_config()` : (defaults fusionnés, préréglages activés).
/// Fichier absent ou JSON invalide : configuration par défaut, sans préréglage.
pub fn load_config(env: &Env) -> (Map<String, Value>, Vec<Map<String, Value>>) {
    let cfg: Value = std::fs::read_to_string(&env.config)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let mut d = default_defaults();
    if let Some(Value::Object(user)) = cfg.get("defaults") {
        for (k, v) in user {
            d.insert(k.clone(), v.clone());
        }
    }
    let presets = match cfg.get("presets") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|p| p.as_object())
            .filter(|p| p.get("enabled").is_none_or(crate::pyval::truthy))
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    (d, presets)
}

/// Le préréglage d'identifiant `id`.
pub fn find_preset<'a>(
    presets: &'a [Map<String, Value>],
    id: &str,
) -> Option<&'a Map<String, Value>> {
    presets.iter().find(|p| {
        p.get("id")
            .is_some_and(|v| matches!(v, Value::String(s) if s == id))
    })
}

/// `out_dir_for(preset_id)` : dossier des vidéos (celui du préréglage, sinon `defaults.dir`).
pub fn out_dir_for(env: &Env, preset_id: Option<&str>) -> String {
    let (d, presets) = load_config(env);
    let p = preset_id.and_then(|id| find_preset(&presets, id));
    let pick =
        |m: &Map<String, Value>| m.get("dir").filter(|v| crate::pyval::truthy(v)).map(py_str);
    let raw = p
        .and_then(pick)
        .or_else(|| pick(&d))
        .unwrap_or_else(|| env.videos.clone());
    let out = env.expanduser(&raw);
    if std::path::Path::new(&out).is_dir() {
        out
    } else {
        env.videos.clone()
    }
}
