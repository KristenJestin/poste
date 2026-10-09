//! `~/.config/poste/display.toml` (`XDG_CONFIG_HOME` respecté) : voir `display.toml.example`.

use anyhow::{bail, Context, Result};
use poste_core::Env;
use serde::Deserialize;
use std::path::PathBuf;

/// Échelle : un nombre (`1.6`, `2`) ou `"auto"`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Scale {
    Int(i64),
    Float(f64),
    Text(String),
}

impl Scale {
    /// Texte pour Hyprland (`1`, `1.6`, `auto`).
    pub fn text(&self) -> String {
        match self {
            Scale::Int(i) => i.to_string(),
            Scale::Float(f) => format!("{f}"),
            Scale::Text(t) => t.clone(),
        }
    }
    pub fn is_text(&self) -> bool {
        matches!(self, Scale::Text(_))
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Internal {
    pub enabled: bool,
    pub mode: Option<String>,
    pub position: Option<String>,
    pub scale: Option<Scale>,
    pub transform: Option<u8>,
}

impl Default for Internal {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: None,
            position: None,
            scale: None,
            transform: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MonitorCfg {
    /// Description EDID (morceau) ou `connector:NOM`.
    #[serde(rename = "match")]
    pub matcher: String,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub scale: Option<Scale>,
    #[serde(default)]
    pub transform: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct WorkspaceCfg {
    /// `3`, `1-10`, `name:mid`.
    pub workspace: String,
    /// `internal`, un critère (EDID, `connector:`).
    pub monitor: String,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub persistent: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Rescue {
    /// Nom de l'espace de travail à vider.
    pub from: String,
    /// Espace de destination.
    pub to: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub detect: Vec<String>,
    #[serde(default)]
    pub detect_any: Vec<String>,
    #[serde(default)]
    pub internal: Option<Internal>,
    #[serde(default)]
    pub monitor: Vec<MonitorCfg>,
    #[serde(default)]
    pub workspace: Vec<WorkspaceCfg>,
    #[serde(default)]
    pub rescue: Vec<Rescue>,
}

impl Profile {
    pub fn label(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.name)
    }
}

fn yes() -> bool {
    true
}
fn internal_name() -> String {
    "eDP-1".into()
}
fn debounce() -> u64 {
    700
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Config {
    #[serde(default = "internal_name")]
    pub internal: String,
    #[serde(default = "yes")]
    pub notify: bool,
    /// Attente (ms) après un événement de branchement avant d'agir : Hyprland doit stabiliser.
    #[serde(default = "debounce")]
    pub debounce_ms: u64,
    #[serde(default)]
    pub profile: Vec<Profile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            internal: internal_name(),
            notify: true,
            debounce_ms: debounce(),
            profile: vec![],
        }
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        let c: Config = toml::from_str(text).context("display.toml invalide")?;
        let mut seen: Vec<&str> = Vec::new();
        for p in &c.profile {
            if p.name.is_empty()
                || !p
                    .name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                bail!(
                    "display.toml : nom de profil invalide « {} » (lettres, chiffres, - et _)",
                    p.name
                );
            }
            if seen.contains(&p.name.as_str()) {
                bail!("display.toml : profil « {} » déclaré deux fois", p.name);
            }
            seen.push(&p.name);
            for m in p.detect.iter().chain(&p.detect_any) {
                if m.trim().is_empty() {
                    bail!("display.toml : profil {} : critère vide", p.name);
                }
            }
        }
        Ok(c)
    }

    pub fn file(env: &Env) -> PathBuf {
        env.config_home.join("poste/display.toml")
    }

    /// Fichier absent : aucune configuration (aucun profil).
    pub fn load(env: &Env) -> Result<Self> {
        let f = Self::file(env);
        match std::fs::read_to_string(&f) {
            Ok(t) => Self::parse(&t).with_context(|| f.display().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("lecture de {}", f.display())),
        }
    }

    pub fn find(&self, name: &str) -> Option<&Profile> {
        self.profile.iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exemple_valide() {
        let c = Config::parse(include_str!("../display.toml.example")).unwrap();
        let names: Vec<_> = c.profile.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["nomade", "boulot", "maison"]);
        let b = c.find("boulot").unwrap();
        assert_eq!(b.monitor.len(), 2);
        assert_eq!(b.monitor[1].transform, Some(1));
        assert_eq!(b.internal.as_ref().unwrap().scale, Some(Scale::Float(1.6)));
        assert_eq!(c.find("maison").unwrap().detect.len(), 3);
    }

    #[test]
    fn echelles() {
        assert_eq!(Scale::Int(2).text(), "2");
        assert_eq!(Scale::Float(1.6).text(), "1.6");
        assert_eq!(Scale::Text("auto".into()).text(), "auto");
    }

    #[test]
    fn refus() {
        assert!(Config::parse("[[profile]]\nname = \"a b\"\n").is_err());
        assert!(Config::parse("[[profile]]\nname = \"a\"\n[[profile]]\nname = \"a\"\n").is_err());
        assert!(Config::parse("[[profile]]\nname = \"a\"\ndetect = [\"\"]\n").is_err());
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }
}
