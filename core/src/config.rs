//! Lecture de `~/.config/poste/config.toml`.

use crate::env::Env;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

/// Configuration de poste. Les clés inconnues sont ignorées : d'autres
/// modules ou étapes ultérieures peuvent en ajouter.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct Config {
    /// Emplacement du dépôt poste (écrit par `./setup`).
    #[serde(default)]
    pub repo: Option<String>,
    /// Dossiers supplémentaires où chercher des modules.
    #[serde(default)]
    pub modules: Vec<String>,
    /// Chemins de `config/` (relatifs à `~`) que cette machine garde en fichiers à elle :
    /// `poste sys setup` ne les lie pas et n'y touche jamais.
    #[serde(default)]
    pub local: Vec<String>,
    /// Modules du dépôt à ne pas installer sur cette machine (retirés s'ils l'étaient).
    #[serde(default)]
    pub disabled: Vec<String>,
    /// Surcharges JSON propres à cette machine (`[[override]]`).
    #[serde(default, rename = "override")]
    pub overrides: Vec<Override>,
}

/// `[[override]]` : un fichier JSON de `config/` qui n'est pas lié mais généré (contenu du dépôt
/// plus ces valeurs). `set` : clés pointées (`"idle.lock" = 3600`) ; une table imbriquée
/// (`idle.lock = 3600` sans guillemets) est aplatie en clés pointées.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct Override {
    pub path: String,
    #[serde(default)]
    pub set: toml::Table,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).context("config.toml invalide")
    }

    /// Charge la configuration ; un fichier absent donne la configuration vide.
    pub fn load(env: &Env) -> Result<Self> {
        let file = env.config_file();
        match std::fs::read_to_string(&file) {
            Ok(text) => Self::parse(&text).with_context(|| file.display().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("lecture de {}", file.display())),
        }
    }

    /// Chemin du dépôt, `~` développé.
    pub fn repo_path(&self, env: &Env) -> Option<PathBuf> {
        self.repo
            .as_deref()
            .filter(|r| !r.is_empty())
            .map(|r| env.expand(r))
    }

    /// Ce module est-il désactivé sur cette machine ?
    pub fn is_disabled(&self, name: &str) -> bool {
        self.disabled.iter().any(|d| d == name)
    }

    /// Dossiers de modules déclarés, `~` développé.
    pub fn module_dirs(&self, env: &Env) -> Vec<PathBuf> {
        self.modules.iter().map(|m| env.expand(m)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_complete() {
        let c =
            Config::parse("repo = \"~/Projects/poste\"\nmodules = [\"~/a\", \"/b\"]\nautre = 1\n")
                .unwrap();
        let e = Env::with_home("/h");
        assert_eq!(c.repo_path(&e), Some(PathBuf::from("/h/Projects/poste")));
        assert_eq!(
            c.module_dirs(&e),
            vec![PathBuf::from("/h/a"), PathBuf::from("/b")]
        );
    }

    #[test]
    fn local_et_disabled() {
        let c = Config::parse("local = [\".config/hypr/monitors.lua\"]\ndisabled = [\"record\"]\n")
            .unwrap();
        assert_eq!(c.local, [".config/hypr/monitors.lua"]);
        assert!(c.is_disabled("record"));
        assert!(!c.is_disabled("music"));
    }

    #[test]
    fn config_vide_et_absente() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
        let dir = tempfile::tempdir().unwrap();
        let e = Env::with_home(dir.path());
        assert_eq!(Config::load(&e).unwrap(), Config::default());
    }

    #[test]
    fn config_invalide() {
        assert!(Config::parse("repo = [").is_err());
    }
}
