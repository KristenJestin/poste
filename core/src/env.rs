//! Runtime environment: home folder, XDG folders and PATH.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Locations poste depends on. Built once, passed to the functions
/// (which allows testing without touching the real environment).
#[derive(Debug, Clone)]
pub struct Env {
    pub home: PathBuf,
    pub data_home: PathBuf,
    pub config_home: PathBuf,
    pub state_home: PathBuf,
    pub cache_home: PathBuf,
    pub path: Vec<PathBuf>,
}

/// Value of an XDG variable if it is set and absolute, otherwise the default.
fn xdg(var: &str, fallback: PathBuf) -> PathBuf {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() && Path::new(&v).is_absolute() => PathBuf::from(v),
        _ => fallback,
    }
}

impl Env {
    /// Reads the process environment (`HOME`, `XDG_*`, `PATH`).
    pub fn from_process() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .context("the HOME variable is not set")?;
        let path = std::env::var_os("PATH")
            .map(|p| {
                std::env::split_paths(&p)
                    .filter(|d| !d.as_os_str().is_empty())
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            data_home: xdg("XDG_DATA_HOME", home.join(".local/share")),
            config_home: xdg("XDG_CONFIG_HOME", home.join(".config")),
            state_home: xdg("XDG_STATE_HOME", home.join(".local/state")),
            cache_home: xdg("XDG_CACHE_HOME", home.join(".cache")),
            home,
            path,
        })
    }

    /// Environment with the default folders under `home` and an empty PATH.
    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        Self {
            data_home: home.join(".local/share"),
            config_home: home.join(".config"),
            state_home: home.join(".local/state"),
            cache_home: home.join(".cache"),
            path: Vec::new(),
            home,
        }
    }

    /// `~/.local/bin`: where the `poste` binary is put.
    pub fn bin_dir(&self) -> PathBuf {
        self.home.join(".local/bin")
    }

    /// `$XDG_DATA_HOME/poste/modules`.
    pub fn modules_dir(&self) -> PathBuf {
        self.data_home.join("poste/modules")
    }

    /// `$XDG_DATA_HOME/poste/versions`: one installed version per folder (`<commit>/`).
    pub fn versions_dir(&self) -> PathBuf {
        self.data_home.join("poste/versions")
    }

    /// `$XDG_DATA_HOME/poste/current`: link to the root everything is read from (an installed
    /// version, or the clone of the repository in dev mode).
    pub fn current_link(&self) -> PathBuf {
        self.data_home.join("poste/current")
    }

    /// `$XDG_CONFIG_HOME/poste/config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.config_home.join("poste/config.toml")
    }

    /// `$XDG_STATE_HOME/poste`.
    pub fn state_dir(&self) -> PathBuf {
        self.state_home.join("poste")
    }

    /// `$XDG_CACHE_HOME/poste`.
    pub fn cache_dir(&self) -> PathBuf {
        self.cache_home.join("poste")
    }

    /// List of what `poste sys setup` put in place.
    pub fn manifest_file(&self) -> PathBuf {
        self.state_dir().join("installed.json")
    }

    /// Backups of the replaced files.
    pub fn backup_dir(&self) -> PathBuf {
        self.state_dir().join("backup")
    }

    /// Expands a leading `~` or `~/…`; a relative path stays relative.
    pub fn expand(&self, s: &str) -> PathBuf {
        if s == "~" {
            self.home.clone()
        } else if let Some(rest) = s.strip_prefix("~/") {
            self.home.join(rest)
        } else {
            PathBuf::from(s)
        }
    }

    /// Shows a path with `~` in place of the home folder.
    pub fn display(&self, p: &Path) -> String {
        match p.strip_prefix(&self.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => p.display().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths() {
        let e = Env::with_home("/h");
        assert_eq!(
            e.modules_dir(),
            PathBuf::from("/h/.local/share/poste/modules")
        );
        assert_eq!(
            e.config_file(),
            PathBuf::from("/h/.config/poste/config.toml")
        );
        assert_eq!(
            e.manifest_file(),
            PathBuf::from("/h/.local/state/poste/installed.json")
        );
        assert_eq!(e.cache_dir(), PathBuf::from("/h/.cache/poste"));
        assert_eq!(
            e.current_link(),
            PathBuf::from("/h/.local/share/poste/current")
        );
        assert_eq!(
            e.versions_dir(),
            PathBuf::from("/h/.local/share/poste/versions")
        );
        assert_eq!(e.bin_dir(), PathBuf::from("/h/.local/bin"));
    }

    #[test]
    fn tilde_expansion() {
        let e = Env::with_home("/h");
        assert_eq!(e.expand("~/a/b"), PathBuf::from("/h/a/b"));
        assert_eq!(e.expand("~"), PathBuf::from("/h"));
        assert_eq!(e.expand("/x"), PathBuf::from("/x"));
        assert_eq!(e.expand("~other"), PathBuf::from("~other"));
    }

    #[test]
    fn display_with_tilde() {
        let e = Env::with_home("/h");
        assert_eq!(e.display(Path::new("/h/a")), "~/a");
        assert_eq!(e.display(Path::new("/etc/a")), "/etc/a");
    }
}
