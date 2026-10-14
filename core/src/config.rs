//! Lecture de `~/.config/poste/config.toml`.

use crate::env::Env;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Réglages de la machine versionnés dans le dépôt de données, à sa racine.
pub const DATA_CONFIG: &str = "poste.toml";

/// Configuration de poste. Les clés inconnues sont ignorées : d'autres
/// modules ou étapes ultérieures peuvent en ajouter.
#[derive(Debug, Default, Clone, Deserialize, PartialEq)]
pub struct Config {
    /// Emplacement du dépôt poste (écrit par `./setup`).
    #[serde(default)]
    pub repo: Option<String>,
    /// D'où cette machine fait tourner poste : une version installée (`release`) ou le clone
    /// du dépôt (`dev`, défaut). Écrit par `poste sys use`, propre à la machine.
    #[serde(default)]
    pub source: Source,
    /// Dépôt des données de cette machine (`poste-data`, privé) : ses fichiers `config/` et ses
    /// étapes `bootstrap/` se posent par-dessus ceux du dépôt, et son `poste.toml` complète
    /// cette config (écrit par `./setup --data`).
    #[serde(default)]
    pub data: Option<String>,
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
    /// Paquets (`bootstrap/packages.toml`) et étapes (`bootstrap/steps.toml`, par `id`) que cette
    /// machine n'installe pas : ni exigés, ni proposés, ni comptés manquants. Rien n'est désinstallé.
    #[serde(default)]
    pub skip: Vec<String>,
    /// Surcharges JSON propres à cette machine (`[[override]]`).
    #[serde(default, rename = "override")]
    pub overrides: Vec<Override>,
}

/// Source de poste sur la machine (clé `source`).
#[derive(Debug, Default, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Une version installée par `poste sys sync` (`~/.local/share/poste/versions/`).
    Release,
    /// Le clone du dépôt (clé `repo`) : fonctionnement d'avant les versions.
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

    /// Charge la configuration ; un fichier absent donne la configuration vide. Avec `data`, le
    /// `poste.toml` du dépôt de données s'y ajoute (listes mises bout à bout, sans doublon).
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
            Err(e) => Err(e).with_context(|| format!("lecture de {}", file.display())),
        }
    }

    /// Ajoute les réglages du dépôt de données ; `repo`, `data` et `source` restent ceux de la machine.
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
        extend(&mut self.skip, more.skip);
        self.overrides.extend(more.overrides);
    }

    /// Chemin du dépôt de données, `~` développé.
    pub fn data_path(&self, env: &Env) -> Option<PathBuf> {
        self.data
            .as_deref()
            .filter(|r| !r.is_empty())
            .map(|r| env.expand(r))
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

    /// Cet identifiant (paquet ou étape du bootstrap) est-il ignoré sur cette machine ?
    pub fn is_skipped(&self, id: &str) -> bool {
        self.skip.iter().any(|d| d == id)
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
    fn skip() {
        let c = Config::parse("skip = [\"gimp\", \"voxtype-model\"]\n").unwrap();
        assert!(c.is_skipped("gimp") && !c.is_skipped("autre"));
        assert!(Config::parse("").unwrap().skip.is_empty());
    }

    #[test]
    fn config_vide_et_absente() {
        assert_eq!(Config::parse("").unwrap(), Config::default());
        let dir = tempfile::tempdir().unwrap();
        let e = Env::with_home(dir.path());
        assert_eq!(Config::load(&e).unwrap(), Config::default());
    }

    #[test]
    fn poste_toml_du_depot_de_donnees() {
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
        // Sans poste.toml : la config de la machine seule.
        let c = Config::load(&e).unwrap();
        assert_eq!(c.data_path(&e), Some(data.clone()));
        assert_eq!(c.disabled, ["power"]);
        std::fs::write(
            data.join(DATA_CONFIG),
            "repo = \"/ailleurs\"\ndisabled = [\"power\", \"record\"]\nlocal = [\".x\"]\n",
        )
        .unwrap();
        let c = Config::load(&e).unwrap();
        assert_eq!(c.repo.as_deref(), Some("~/poste"));
        assert_eq!(c.disabled, ["power", "record"]);
        assert_eq!(c.local, [".x"]);
    }

    #[test]
    fn source() {
        assert_eq!(Config::parse("").unwrap().source, Source::Dev);
        assert_eq!(
            Config::parse("source = \"release\"\n").unwrap().source,
            Source::Release
        );
        assert!(Config::parse("source = \"autre\"\n").is_err());
    }

    #[test]
    fn config_invalide() {
        assert!(Config::parse("repo = [").is_err());
    }
}
