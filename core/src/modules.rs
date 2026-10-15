//! Module discovery and reading of the `module.toml` cards.
//!
//! Search order, the first hit wins:
//! 1. `<data>/poste/modules/<name>/poste-<name>`
//! 2. each folder of `modules` in config.toml (`poste-<name>` or `<name>/poste-<name>`)
//! 3. the `PATH`

use crate::{config::Config, env::Env};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// `module.toml` card of a module.
#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
pub struct Card {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub commands: Vec<CardCommand>,
    /// Lines to put in files that have no include (`[[line]]`).
    #[serde(default)]
    pub line: Vec<CardLine>,
    /// User systemd units of the module (files in `systemd/`), enabled by
    /// `poste sys setup` and disabled when the module is removed.
    #[serde(default)]
    pub units: Vec<String>,
    /// Program the module plugs into the desktop (looked up in the `PATH`): without it,
    /// `poste sys setup` leaves the module out.
    #[serde(default)]
    pub requires: Option<String>,
}

/// `[[line]]` of a card: a line a module wants to see in a file of `~`.
/// `file` accepts `~/…`; `line` accepts `{home}` (home folder) and `{modules}` (folder of the installed modules).
#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
pub struct CardLine {
    pub file: String,
    pub line: String,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
pub struct CardCommand {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

impl Card {
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).context("invalid module.toml")
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&text).with_context(|| path.display().to_string())
    }
}

/// Where a module that was found comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    /// `~/.local/share/poste/modules`
    Data,
    /// A folder declared in `modules` of config.toml.
    Config(PathBuf),
    /// A folder of the `PATH`.
    Path(PathBuf),
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub program: PathBuf,
    pub origin: Origin,
    pub card: Option<Card>,
}

impl Module {
    pub fn description(&self) -> &str {
        self.card
            .as_ref()
            .map(|c| c.description.as_str())
            .unwrap_or("")
    }
}

/// A valid module name cannot designate a path or an option.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !name.starts_with('-')
}

#[cfg(unix)]
fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(p: &Path) -> bool {
    p.is_file()
}

/// The module `name`: first executable program found in the search order.
pub fn find(env: &Env, cfg: &Config, name: &str) -> Option<Module> {
    if !valid_name(name) {
        return None;
    }
    let prog = format!("poste-{name}");
    let make = |program: PathBuf, origin: Origin| {
        let card = Card::load(&program.with_file_name("module.toml")).ok();
        Module {
            name: name.to_string(),
            program,
            origin,
            card,
        }
    };

    let p = env.modules_dir().join(name).join(&prog);
    if is_executable(&p) {
        return Some(make(p, Origin::Data));
    }
    for dir in cfg.module_dirs(env) {
        for p in [dir.join(&prog), dir.join(name).join(&prog)] {
            if is_executable(&p) {
                return Some(make(p, Origin::Config(dir.clone())));
            }
        }
    }
    for dir in &env.path {
        let p = dir.join(&prog);
        if is_executable(&p) {
            return Some(make(p, Origin::Path(dir.clone())));
        }
    }
    None
}

/// Candidate names found in a folder (files `poste-X`, and sub-folders `X`
/// if `nested`).
fn scan(dir: &Path, nested: bool, out: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let fname = e.file_name().to_string_lossy().into_owned();
        let is_dir = std::fs::metadata(e.path())
            .map(|m| m.is_dir())
            .unwrap_or(false);
        if is_dir {
            if nested {
                out.insert(fname);
            }
        } else if let Some(n) = fname.strip_prefix("poste-") {
            out.insert(n.to_string());
        }
    }
}

/// All the available modules, sorted by name, each resolved like `find`.
pub fn list(env: &Env, cfg: &Config) -> Vec<Module> {
    let mut names = BTreeSet::new();
    scan_data(env, &mut names);
    for dir in cfg.module_dirs(env) {
        scan(&dir, true, &mut names);
    }
    for dir in &env.path {
        scan(dir, false, &mut names);
    }
    names.iter().filter_map(|n| find(env, cfg, n)).collect()
}

fn scan_data(env: &Env, out: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(env.modules_dir()) else {
        return;
    };
    for e in entries.flatten() {
        if e.path().is_dir() {
            out.insert(e.file_name().to_string_lossy().into_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[cfg(unix)]
    fn script(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "#!/bin/sh\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn valid_names() {
        assert!(valid_name("sys"));
        assert!(valid_name("a-b_1"));
        assert!(!valid_name(""));
        assert!(!valid_name("../x"));
        assert!(!valid_name("--help"));
        assert!(!valid_name("a/b"));
    }

    #[test]
    fn card() {
        let c = Card::parse(
            "name = \"x\"\ndescription = \"d\"\ncommands = [{ name = \"a\", description = \"b\" }, { name = \"c\" }]\n",
        )
        .unwrap();
        assert_eq!(c.name.as_deref(), Some("x"));
        assert_eq!(c.commands.len(), 2);
        assert_eq!(c.commands[1].description, "");
        assert_eq!(Card::parse("").unwrap(), Card::default());
    }

    #[cfg(unix)]
    #[test]
    fn search_order() {
        let t = tempfile::tempdir().unwrap();
        let mut env = Env::with_home(t.path().join("home"));
        let ext = t.path().join("ext");
        let bin = t.path().join("bin");
        env.path = vec![bin.clone()];
        let cfg = Config {
            repo: None,
            modules: vec![ext.to_string_lossy().into_owned()],
            ..Default::default()
        };

        // Present in all three places: the data one wins.
        script(&env.modules_dir().join("m/poste-m"));
        script(&ext.join("m/poste-m"));
        script(&bin.join("poste-m"));
        assert_eq!(find(&env, &cfg, "m").unwrap().origin, Origin::Data);

        // Config before PATH, direct form and folder form.
        script(&ext.join("poste-c"));
        script(&bin.join("poste-c"));
        assert!(matches!(
            find(&env, &cfg, "c").unwrap().origin,
            Origin::Config(_)
        ));
        script(&ext.join("d/poste-d"));
        assert!(matches!(
            find(&env, &cfg, "d").unwrap().origin,
            Origin::Config(_)
        ));

        script(&bin.join("poste-p"));
        assert!(matches!(
            find(&env, &cfg, "p").unwrap().origin,
            Origin::Path(_)
        ));
        assert!(find(&env, &cfg, "absent").is_none());
        assert!(find(&env, &cfg, "../m").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn list_and_cards() {
        let t = tempfile::tempdir().unwrap();
        let mut env = Env::with_home(t.path().join("home"));
        let bin = t.path().join("bin");
        env.path = vec![bin.clone()];
        let cfg = Config::default();

        script(&env.modules_dir().join("b/poste-b"));
        fs::write(
            env.modules_dir().join("b/module.toml"),
            "description = \"The B\"\n",
        )
        .unwrap();
        script(&bin.join("poste-a"));
        // Not executable: ignored.
        fs::write(bin.join("poste-z"), "x").unwrap();
        // Folder without a program: ignored.
        fs::create_dir_all(env.modules_dir().join("empty")).unwrap();

        let l = list(&env, &cfg);
        let names: Vec<_> = l.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        assert_eq!(l[1].description(), "The B");
        assert_eq!(l[0].description(), "");
    }
}
