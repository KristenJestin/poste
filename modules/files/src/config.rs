//! Settings, in `~/.config/poste/` (`XDG_CONFIG_HOME` respected), both optional:
//!
//! - `files.toml`, missing keys at their default value:
//!
//!   ```toml
//!   roots = ["~"]   # indexed folders (default: the home folder)
//!   limit = 30      # results given to the launcher
//!   ```
//!
//!
//! The exclusions of a machine are data, not settings: `files/ignore` in the machine's data
//! repository (`poste-data`, key `data` of `config.toml`), then
//! `$XDG_DATA_HOME/poste/files/ignore` (local, not versioned), both in .gitignore syntax and
//! read after `default.ignore`.

use anyhow::{bail, Context, Result};
use poste_core::Env;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub roots: Vec<String>,
    pub limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            roots: vec!["~".into()],
            limit: 30,
        }
    }
}

/// Reads `path`; `None` when it does not exist.
fn read_optional(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        let c: Config = toml::from_str(text).context("invalid files.toml")?;
        if c.roots.is_empty() {
            bail!("files.toml: roots must hold at least one folder");
        }
        if c.limit == 0 || c.limit > 200 {
            bail!("files.toml: limit must be between 1 and 200");
        }
        Ok(c)
    }

    pub fn file(env: &Env) -> PathBuf {
        env.config_home.join("poste/files.toml")
    }

    /// Exclusion files of the machine, in reading order: the data repository's (when the
    /// machine has one), then the local one. Each may be missing.
    pub fn ignore_files(env: &Env) -> Vec<PathBuf> {
        let data = poste_core::Config::load(env)
            .ok()
            .and_then(|c| c.data_path(env))
            .map(|d| d.join("files/ignore"));
        data.into_iter()
            .chain([env.data_home.join("poste/files/ignore")])
            .collect()
    }

    /// Loads files.toml; missing, the defaults.
    pub fn load(env: &Env) -> Result<Self> {
        let f = Self::file(env);
        match read_optional(&f)? {
            Some(t) => Self::parse(&t).with_context(|| f.display().to_string()),
            None => Ok(Self::default()),
        }
    }

    /// The exclusion files that exist: (name shown in errors, text).
    pub fn load_ignores(env: &Env) -> Result<Vec<(String, String)>> {
        let mut out = Vec::new();
        for f in Self::ignore_files(env) {
            if let Some(text) = read_optional(&f)? {
                out.push((env.display(&f), text));
            }
        }
        Ok(out)
    }

    /// Absolute roots, without duplicates. A root inside another is kept: the exclusions may
    /// stop the walk of the outer one before it (`~/.local/share/notes` under `~`, where `.*`
    /// excludes `.local`); the entries found twice are merged in the index.
    pub fn root_paths(&self, env: &Env) -> Result<Vec<PathBuf>> {
        let mut out: Vec<PathBuf> = Vec::new();
        for r in &self.roots {
            let p = env.expand(r);
            if !p.is_absolute() {
                bail!("files.toml: root {r} is not an absolute path (nor ~/…)");
            }
            out.push(p);
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_partial_override() {
        let c = Config::parse("").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.roots, ["~"]);
        let c = Config::parse("limit = 50\n").unwrap();
        assert_eq!(c.limit, 50);
        assert_eq!(c.roots, ["~"]);
    }

    #[test]
    fn inconsistent_values_refused() {
        assert!(Config::parse("roots = []\n").is_err());
        assert!(Config::parse("limit = 0\n").is_err());
        assert!(Config::parse("racines = [\"~\"]\n").is_err());
        // Exclusions are data: files/ignore, not a key here.
        assert!(Config::parse("exclude = [\"*.iso\"]\n").is_err());
    }

    #[test]
    fn roots_expanded_and_deduplicated() {
        let env = Env::with_home("/h");
        let c = Config::parse("roots = [\"~/Projects\", \"~\", \"/srv/data\", \"~\"]\n").unwrap();
        assert_eq!(
            c.root_paths(&env).unwrap(),
            [
                PathBuf::from("/h"),
                PathBuf::from("/h/Projects"),
                PathBuf::from("/srv/data")
            ]
        );
        let c = Config::parse("roots = [\"relative\"]\n").unwrap();
        assert!(c.root_paths(&env).is_err());
    }
}
