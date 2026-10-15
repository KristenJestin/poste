//! Settings, in `~/.config/poste/` (`XDG_CONFIG_HOME` respected), both optional:
//!
//! - `files.toml`, missing keys at their default value:
//!
//!   ```toml
//!   roots = ["~"]   # indexed folders (default: the home folder)
//!   limit = 30      # results given to the launcher
//!   ```
//!
//! - `files.ignore`: exclusions in .gitignore syntax, read after `default.ignore`.

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

    pub fn ignore_file(env: &Env) -> PathBuf {
        env.config_home.join("poste/files.ignore")
    }

    /// Loads files.toml; missing, the defaults.
    pub fn load(env: &Env) -> Result<Self> {
        let f = Self::file(env);
        match read_optional(&f)? {
            Some(t) => Self::parse(&t).with_context(|| f.display().to_string()),
            None => Ok(Self::default()),
        }
    }

    /// Text of files.ignore (empty when it does not exist).
    pub fn load_ignore(env: &Env) -> Result<String> {
        Ok(read_optional(&Self::ignore_file(env))?.unwrap_or_default())
    }

    /// Absolute roots, without duplicates or a root inside another.
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
        let all = out.clone();
        out.retain(|p| !all.iter().any(|o| o != p && p.starts_with(o)));
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
        // Exclusions go in files.ignore.
        assert!(Config::parse("exclude = [\"*.iso\"]\n").is_err());
    }

    #[test]
    fn roots_expanded_and_not_nested() {
        let env = Env::with_home("/h");
        let c = Config::parse("roots = [\"~/Projects\", \"~\", \"/srv/data\", \"~\"]\n").unwrap();
        assert_eq!(
            c.root_paths(&env).unwrap(),
            [PathBuf::from("/h"), PathBuf::from("/srv/data")]
        );
        let c = Config::parse("roots = [\"relative\"]\n").unwrap();
        assert!(c.root_paths(&env).is_err());
    }
}
