//! Reading of `~/.config/poste/config.toml`.

use crate::env::Env;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Machine settings versioned in the data repository, at its root.
pub const DATA_CONFIG: &str = "poste.toml";

/// Configuration of poste. Unknown keys are ignored: other
/// modules or later steps may add some.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct Config {
    /// Location of the poste repository (written by `./setup`).
    #[serde(default)]
    pub repo: Option<String>,
    /// Where this machine runs poste from: an installed version (`release`) or the clone
    /// of the repository (`dev`, default). Written by `poste sys use`, specific to the machine.
    #[serde(default)]
    pub source: Source,
    /// Data repository of this machine (`poste-data`, private): its `config/` files and its
    /// `bootstrap/` steps are put in place on top of those of the repository, and its `poste.toml` completes
    /// this config (written by `./setup --data`).
    #[serde(default)]
    pub data: Option<String>,
    /// Extra folders to look for modules in.
    #[serde(default)]
    pub modules: Vec<String>,
    /// Paths of `config/` (relative to `~`) that this machine keeps as files of its own:
    /// `poste sys setup` does not link them and never touches them.
    #[serde(default)]
    pub local: Vec<String>,
    /// Modules of the repository not to install on this machine (removed if they were).
    #[serde(default)]
    pub disabled: Vec<String>,
    /// Add-ons of the repository (`addons/<name>/`) this machine uses: their bootstrap steps
    /// install the program, and the add-on is plugged in once the program is there.
    #[serde(default)]
    pub addons: Vec<String>,
    /// Packages (`bootstrap/packages.toml`) and steps (`bootstrap/steps.toml`, by `id`) that this
    /// machine does not install: neither required, nor offered, nor counted as missing. Nothing is uninstalled.
    #[serde(default)]
    pub skip: Vec<String>,
    /// JSON overrides specific to this machine (`[[override]]`).
    #[serde(default, rename = "override")]
    pub overrides: Vec<Override>,
}

/// Source of poste on the machine (key `source`).
#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// A version installed by `poste sys sync` (`~/.local/share/poste/versions/`).
    Release,
    /// The clone of the repository (key `repo`): how it worked before versions.
    #[default]
    Dev,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Release => "release",
            Source::Dev => "dev",
        }
    }
}

/// `[[override]]`: a JSON file of `config/` that is not linked but generated (content of the repository
/// plus these values). `set`: dotted keys (`"idle.lock" = 3600`); a nested table
/// (`idle.lock = 3600` without quotes) is flattened into dotted keys.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct Override {
    pub path: String,
    #[serde(default)]
    pub set: toml::Table,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).context("invalid config.toml")
    }

    /// Loads the configuration; a missing file gives the empty configuration. With `data`, the
    /// `poste.toml` of the data repository is added to it (lists put end to end, without duplicates).
    pub fn load(env: &Env) -> Result<Self> {
        let mut cfg = Self::load_file(&env.config_file())?.unwrap_or_default();
        if let Some(data) = cfg.data_path(env) {
            if let Some(more) = Self::load_file(&data.join(DATA_CONFIG))? {
                cfg.merge(more);
            }
        }
        Ok(cfg)
    }

    fn load_file(file: &Path) -> Result<Option<Self>> {
        match std::fs::read_to_string(file) {
            Ok(text) => Self::parse(&text)
                .with_context(|| file.display().to_string())
                .map(Some),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("reading {}", file.display())),
        }
    }

    /// Adds the settings of the data repository; `repo`, `data` and `source` stay those of the machine.
    fn merge(&mut self, more: Self) {
        fn extend(a: &mut Vec<String>, b: Vec<String>) {
            for v in b {
                if !a.contains(&v) {
                    a.push(v);
                }
            }
        }
        extend(&mut self.modules, more.modules);
        extend(&mut self.local, more.local);
        extend(&mut self.disabled, more.disabled);
        extend(&mut self.addons, more.addons);
        extend(&mut self.skip, more.skip);
        self.overrides.extend(more.overrides);
    }

    /// Path of the data repository, `~` expanded.
    pub fn data_path(&self, env: &Env) -> Option<PathBuf> {
        self.data
            .as_deref()
            .filter(|r| !r.is_empty())
            .map(|r| env.expand(r))
    }

    /// Path of the repository, `~` expanded.
    pub fn repo_path(&self, env: &Env) -> Option<PathBuf> {
        self.repo
            .as_deref()
            .filter(|r| !r.is_empty())
            .map(|r| env.expand(r))
    }

    /// Is this module disabled on this machine?
    pub fn is_disabled(&self, name: &str) -> bool {
        self.disabled.iter().any(|d| d == name)
    }

    /// Is this identifier (bootstrap package or step) ignored on this machine?
    pub fn is_skipped(&self, id: &str) -> bool {
        self.skip.iter().any(|d| d == id)
    }

    /// Declared module folders, `~` expanded.
    pub fn module_dirs(&self, env: &Env) -> Vec<PathBuf> {
        self.modules.iter().map(|m| env.expand(m)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_config() {
        let c =
            Config::parse("repo = \"~/Projects/poste\"\nmodules = [\"~/a\", \"/b\"]\nother = 1\n")
                .unwrap();
        let e = Env::with_home("/h");
        assert_eq!(c.repo_path(&e), Some(PathBuf::from("/h/Projects/poste")));
        assert_eq!(
            c.module_dirs(&e),
            vec![PathBuf::from("/h/a"), PathBuf::from("/b")]
        );
    }

    #[test]
    fn local_and_disabled() {
        let c = Config::parse("local = [\".config/hypr/monitors.lua\"]\ndisabled = [\"record\"]\n")
            .unwrap();
        assert_eq!(c.local, [".config/hypr/monitors.lua"]);
        assert!(c.is_disabled("record"));
        assert!(!c.is_disabled("music"));
    }

    #[test]
    fn skip() {
        let c = Config::parse("skip = [\"gimp\", \"voxtype-model\"]\n").unwrap();
        assert!(c.is_skipped("gimp") && !c.is_skipped("other"));
        assert!(Config::parse("").unwrap().skip.is_empty());
    }

    #[test]
    fn empty_and_missing_config() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
        let dir = tempfile::tempdir().unwrap();
        let e = Env::with_home(dir.path());
        assert_eq!(Config::load(&e).unwrap(), Config::default());
    }

    #[test]
    fn poste_toml_of_the_data_repository() {
        let dir = tempfile::tempdir().unwrap();
        let e = Env::with_home(dir.path());
        let data = dir.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(e.config_file().parent().unwrap()).unwrap();
        std::fs::write(
            e.config_file(),
            "repo = \"~/poste\"\ndata = \"~/data\"\ndisabled = [\"power\"]\n",
        )
        .unwrap();
        // Without poste.toml: the machine's config alone.
        let c = Config::load(&e).unwrap();
        assert_eq!(c.data_path(&e), Some(data.clone()));
        assert_eq!(c.disabled, ["power"]);
        std::fs::write(
            data.join(DATA_CONFIG),
            "repo = \"/elsewhere\"\ndisabled = [\"power\", \"record\"]\nlocal = [\".x\"]\naddons = [\"player\"]\n",
        )
        .unwrap();
        let c = Config::load(&e).unwrap();
        assert_eq!(c.repo.as_deref(), Some("~/poste"));
        assert_eq!(c.disabled, ["power", "record"]);
        assert_eq!(c.local, [".x"]);
        assert_eq!(c.addons, ["player"]);
    }

    #[test]
    fn source() {
        assert_eq!(Config::parse("").unwrap().source, Source::Dev);
        assert_eq!(
            Config::parse("source = \"release\"\n").unwrap().source,
            Source::Release
        );
        assert!(Config::parse("source = \"other\"\n").is_err());
    }

    #[test]
    fn invalid_config() {
        assert!(Config::parse("repo = [").is_err());
    }
}
