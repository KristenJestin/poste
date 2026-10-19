//! The Unity projects of the machine: the ones Unity Hub knows (`unityhub/projects-v1.json`)
//! plus the ones found under the roots of `~/.config/poste/unity.toml` (a folder holding
//! `ProjectSettings/ProjectVersion.txt`), at any depth up to `depth`.

use anyhow::{bail, Context, Result};
use poste_core::Env;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// `~/.config/poste/unity.toml`, optional, missing keys at their default value:
///
/// ```toml
/// roots = ["~/Projects", "~/Unity"]   # folders searched for projects
/// depth = 4                           # folder levels under a root
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub roots: Vec<String>,
    pub depth: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            roots: vec!["~/Projects".into(), "~/Unity".into()],
            depth: 4,
        }
    }
}

impl Config {
    pub fn file(env: &Env) -> PathBuf {
        env.config_home.join("poste/unity.toml")
    }

    pub fn parse(text: &str) -> Result<Self> {
        let c: Config = toml::from_str(text).context("invalid unity.toml")?;
        if c.depth == 0 || c.depth > 8 {
            bail!("unity.toml: depth must be between 1 and 8");
        }
        Ok(c)
    }

    pub fn load(env: &Env) -> Result<Self> {
        let path = Self::file(env);
        match std::fs::read_to_string(&path) {
            Ok(t) => Self::parse(&t),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Project {
    pub path: PathBuf,
    pub title: String,
    pub version: String,
    /// Milliseconds since the epoch: last opening for Hub, last write otherwise.
    pub modified: u64,
    pub favorite: bool,
}

/// Folders never opened by the walk: a Unity or build output, dependencies.
const SKIPPED: [&str; 9] = [
    "Library",
    "Temp",
    "Logs",
    "obj",
    "node_modules",
    "target",
    "build",
    "dist",
    "vendor",
];

fn version_of(path: &Path) -> String {
    std::fs::read_to_string(path.join("ProjectSettings/ProjectVersion.txt"))
        .ok()
        .and_then(|t| {
            t.lines()
                .find_map(|l| l.strip_prefix("m_EditorVersion:"))
                .map(|v| v.trim().to_string())
        })
        .unwrap_or_default()
}

fn mtime_ms(path: &Path) -> Option<u64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(UNIX_EPOCH).ok()?.as_millis() as u64)
}

/// When the editor last touched the project: its settings, written when it closes, and
/// `Library`, written while it runs.
fn last_write(path: &Path) -> u64 {
    [
        "ProjectSettings/ProjectVersion.txt",
        "UserSettings/EditorUserSettings.asset",
        "Library",
    ]
    .iter()
    .filter_map(|f| mtime_ms(&path.join(f)))
    .max()
    .unwrap_or(0)
}

fn is_project(dir: &Path) -> bool {
    dir.join("ProjectSettings/ProjectVersion.txt").is_file()
}

/// The projects under `dir`, `depth` levels at most. A project is not searched inside;
/// hidden folders and `SKIPPED` are never opened. A symbolic link to a folder is followed
/// (a project kept on another disk); the depth bound stops a loop.
fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if is_project(dir) {
        out.push(dir.to_path_buf());
        return;
    }
    if depth == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') || SKIPPED.contains(&name.as_ref()) {
            continue;
        }
        let path = e.path();
        if path.is_dir() {
            walk(&path, depth - 1, out);
        }
    }
}

/// The projects registered in Unity Hub that still exist.
fn from_hub(env: &Env) -> Vec<Project> {
    let file = env.config_home.join("unityhub/projects-v1.json");
    let Ok(text) = std::fs::read_to_string(file) else {
        return Vec::new();
    };
    let Ok(doc) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    let Some(map) = doc.get("data").unwrap_or(&doc).as_object() else {
        return Vec::new();
    };
    map.values()
        .filter_map(|p| {
            let path = PathBuf::from(p.get("path")?.as_str()?);
            if !path.is_dir() {
                return None;
            }
            let title = p.get("title").and_then(Value::as_str).unwrap_or("");
            let version = p.get("version").and_then(Value::as_str).unwrap_or("");
            Some(Project {
                title: if title.is_empty() {
                    file_name(&path)
                } else {
                    title.to_string()
                },
                version: if version.is_empty() {
                    version_of(&path)
                } else {
                    version.to_string()
                },
                modified: p.get("lastModified").and_then(Value::as_u64).unwrap_or(0),
                favorite: p.get("isFavorite").and_then(Value::as_bool) == Some(true),
                path,
            })
        })
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// Every project once (Hub's entry when it knows it), favorites first, then the newest.
pub fn list(env: &Env, cfg: &Config) -> Vec<Project> {
    let mut by_key: HashMap<PathBuf, Project> = HashMap::new();
    let key = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    // Two Hub entries for one folder (a symbolic link): the one opened last.
    for p in from_hub(env) {
        match by_key.entry(key(&p.path)) {
            std::collections::hash_map::Entry::Occupied(mut e) => {
                if p.modified > e.get().modified {
                    e.insert(p);
                }
            }
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(p);
            }
        }
    }
    let mut found = Vec::new();
    for root in &cfg.roots {
        walk(&env.expand(root), cfg.depth, &mut found);
    }
    for path in found {
        by_key.entry(key(&path)).or_insert_with(|| Project {
            title: file_name(&path),
            version: version_of(&path),
            modified: last_write(&path),
            favorite: false,
            path,
        });
    }
    let mut out: Vec<Project> = by_key.into_values().collect();
    out.sort_by(|a, b| {
        b.favorite
            .cmp(&a.favorite)
            .then(b.modified.cmp(&a.modified))
            .then_with(|| a.title.cmp(&b.title))
    });
    out
}

/// A launcher entry (contract: `docs/launcher-format.md`). Enter opens the project with
/// `unity open`, which picks the editor of its version; the launcher adds "Copy path" itself.
pub fn to_json(env: &Env, p: &Project) -> Value {
    let path = p.path.to_string_lossy();
    let short = env.display(&p.path);
    let quoted = poste_core::shell::word(&path);
    let subtitle = [p.version.as_str(), short.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" · ");
    json!({
        "id": path,
        "name": p.title,
        "subtitle": subtitle,
        "icon": "unityhub",
        "verb": "Open",
        "keywords": [p.version, short],
        "path": path,
        "action": {"type": "command", "command": format!("unity open {quoted}")},
        "actions": [
            {"label": "Open folder", "icon": "\u{f024b}",
             "action": {"type": "command", "command": format!("xdg-open {quoted}")}},
            {"label": "Open terminal here", "icon": "\u{f018d}",
             "action": {"type": "terminal", "command": "bash", "cwd": path}}
        ]
    })
}

/// A single launcher line without action, to say what went wrong.
pub fn notice(title: &str, detail: &str) -> Value {
    json!({"id": "unity-error", "name": title, "subtitle": detail, "icon": "dialog-error"})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn project(home: &Path, rel: &str, version: &str) -> PathBuf {
        let p = home.join(rel);
        fs::create_dir_all(p.join("ProjectSettings")).unwrap();
        fs::write(
            p.join("ProjectSettings/ProjectVersion.txt"),
            format!("m_EditorVersion: {version}\nm_EditorVersionWithRevision: {version} (abc)\n"),
        )
        .unwrap();
        p
    }

    #[test]
    fn config_defaults_and_bounds() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
        assert_eq!(Config::parse("depth = 2").unwrap().depth, 2);
        assert!(Config::parse("depth = 0").is_err());
        assert!(Config::parse("rootz = []").is_err());
    }

    #[test]
    fn finds_nested_projects_and_skips_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        project(home, "Projects/game", "6000.0.1f1");
        project(home, "Projects/work/_worktrees/kit", "6000.5.4f1");
        // Not searched: inside a project, hidden, skipped, too deep.
        project(home, "Projects/game/Packages/inner", "1");
        project(home, "Projects/.hidden/game", "1");
        project(home, "Projects/site/node_modules/game", "1");
        project(home, "Projects/a/b/c/d/e", "1");
        let env = Env::with_home(home);
        let cfg = Config::default();
        let mut got: Vec<String> = list(&env, &cfg)
            .iter()
            .map(|p| env.display(&p.path))
            .collect();
        got.sort();
        assert_eq!(got, ["~/Projects/game", "~/Projects/work/_worktrees/kit"]);
    }

    #[test]
    fn hub_entry_wins_and_favorites_come_first() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let a = project(home, "Projects/a", "6000.0.1f1");
        let b = project(home, "Projects/b", "6000.0.1f1");
        fs::create_dir_all(home.join(".config/unityhub")).unwrap();
        let hub = json!({"schema_version": "v1", "data": {
            a.to_string_lossy(): {"path": a, "title": "Alpha", "version": "6000.9.9f1", "lastModified": 5, "isFavorite": true},
            "/nowhere/gone": {"path": "/nowhere/gone", "title": "Gone"}
        }});
        fs::write(
            home.join(".config/unityhub/projects-v1.json"),
            hub.to_string(),
        )
        .unwrap();
        let env = Env::with_home(home);
        let got = list(&env, &Config::default());
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].title, "Alpha");
        assert_eq!(got[0].version, "6000.9.9f1");
        assert!(got[0].favorite);
        assert_eq!(got[1].path, b);
    }

    #[test]
    fn launcher_entry() {
        let env = Env::with_home("/home/u");
        let p = Project {
            path: "/home/u/Projects/it's".into(),
            title: "it's".into(),
            version: "6000.5.4f1".into(),
            modified: 0,
            favorite: false,
        };
        let v = to_json(&env, &p);
        assert_eq!(v["name"], "it's");
        assert_eq!(v["verb"], "Open");
        assert_eq!(v["subtitle"], "6000.5.4f1 · ~/Projects/it's");
        assert_eq!(
            v["action"]["command"],
            "unity open '/home/u/Projects/it'\\''s'"
        );
        assert_eq!(v["path"], "/home/u/Projects/it's");
    }
}
