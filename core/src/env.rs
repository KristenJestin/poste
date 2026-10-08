//! Environnement d'exécution : dossier personnel, dossiers XDG et PATH.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Emplacements dont dépend poste. Construit une fois, passé aux fonctions
/// (ce qui permet de tester sans toucher au vrai environnement).
#[derive(Debug, Clone)]
pub struct Env {
    pub home: PathBuf,
    pub data_home: PathBuf,
    pub config_home: PathBuf,
    pub state_home: PathBuf,
    pub path: Vec<PathBuf>,
}

/// Valeur d'une variable XDG si elle est définie et absolue, sinon le défaut.
fn xdg(var: &str, fallback: PathBuf) -> PathBuf {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() && Path::new(&v).is_absolute() => PathBuf::from(v),
        _ => fallback,
    }
}

impl Env {
    /// Lit l'environnement du processus (`HOME`, `XDG_*`, `PATH`).
    pub fn from_process() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .context("la variable HOME n'est pas définie")?;
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
            home,
            path,
        })
    }

    /// Environnement avec les dossiers par défaut sous `home` et un PATH vide.
    pub fn with_home(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        Self {
            data_home: home.join(".local/share"),
            config_home: home.join(".config"),
            state_home: home.join(".local/state"),
            path: Vec::new(),
            home,
        }
    }

    /// `~/.local/bin` : où est posé le binaire `poste`.
    pub fn bin_dir(&self) -> PathBuf {
        self.home.join(".local/bin")
    }

    /// `$XDG_DATA_HOME/poste/modules`.
    pub fn modules_dir(&self) -> PathBuf {
        self.data_home.join("poste/modules")
    }

    /// `$XDG_CONFIG_HOME/poste/config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.config_home.join("poste/config.toml")
    }

    /// `$XDG_STATE_HOME/poste`.
    pub fn state_dir(&self) -> PathBuf {
        self.state_home.join("poste")
    }

    /// Liste de ce qui a été posé par `poste sys setup`.
    pub fn manifest_file(&self) -> PathBuf {
        self.state_dir().join("installed.json")
    }

    /// Sauvegardes des fichiers remplacés.
    pub fn backup_dir(&self) -> PathBuf {
        self.state_dir().join("backup")
    }

    /// Développe un `~` ou `~/…` initial ; un chemin relatif reste relatif.
    pub fn expand(&self, s: &str) -> PathBuf {
        if s == "~" {
            self.home.clone()
        } else if let Some(rest) = s.strip_prefix("~/") {
            self.home.join(rest)
        } else {
            PathBuf::from(s)
        }
    }

    /// Affiche un chemin avec `~` à la place du dossier personnel.
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
    fn chemins_par_defaut() {
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
        assert_eq!(e.bin_dir(), PathBuf::from("/h/.local/bin"));
    }

    #[test]
    fn expansion_du_tilde() {
        let e = Env::with_home("/h");
        assert_eq!(e.expand("~/a/b"), PathBuf::from("/h/a/b"));
        assert_eq!(e.expand("~"), PathBuf::from("/h"));
        assert_eq!(e.expand("/x"), PathBuf::from("/x"));
        assert_eq!(e.expand("~autre"), PathBuf::from("~autre"));
    }

    #[test]
    fn affichage_avec_tilde() {
        let e = Env::with_home("/h");
        assert_eq!(e.display(Path::new("/h/a")), "~/a");
        assert_eq!(e.display(Path::new("/etc/a")), "/etc/a");
    }
}
