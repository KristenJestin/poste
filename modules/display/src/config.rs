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
pub struct Rescue {
    /// Espaces à vider : un numéro (`3`), une plage (`1-10`), comparés au numéro de l'espace, ou un
    /// nom (`mid`), comparé à son nom.
    pub from: String,
    /// Destination : un numéro, ou une plage de même longueur que `from` (`1-10` vers `21-30` :
    /// 1 va sur 21, 2 sur 22…).
    pub to: String,
}

/// Espace à vider, désigné par son numéro ou par son nom.
#[derive(Debug, Clone, PartialEq)]
pub enum RescueFrom {
    Id(i64),
    Name(String),
}

/// `3` ou `1-10` en numéros ; `None` pour un nom.
fn ids(spec: &str) -> Option<Vec<i64>> {
    let s = spec.trim();
    if let Ok(n) = s.parse::<i64>() {
        return Some(vec![n]);
    }
    let (a, b) = s.split_once('-')?;
    let (a, b) = (a.trim().parse::<i64>().ok()?, b.trim().parse::<i64>().ok()?);
    (a <= b && b - a < 100).then(|| (a..=b).collect())
}

impl Rescue {
    /// Les paires (espace à vider, destination), ou l'erreur à montrer.
    pub fn pairs(&self) -> Result<Vec<(RescueFrom, String)>, String> {
        let to = ids(&self.to).ok_or_else(|| {
            format!(
                "rescue : destination « {} » : un numéro ou une plage (21-30)",
                self.to
            )
        })?;
        let Some(from) = ids(&self.from) else {
            if to.len() != 1 {
                return Err(format!(
                    "rescue : « {} » est un nom, la destination doit être un seul numéro",
                    self.from
                ));
            }
            return Ok(vec![(
                RescueFrom::Name(self.from.trim().into()),
                to[0].to_string(),
            )]);
        };
        if to.len() != 1 && to.len() != from.len() {
            return Err(format!(
                "rescue : « {} » vers « {} » : plages de longueurs différentes",
                self.from, self.to
            ));
        }
        Ok(from
            .iter()
            .enumerate()
            .map(|(i, &f)| {
                let t = if to.len() == 1 { to[0] } else { to[i] };
                (RescueFrom::Id(f), t.to_string())
            })
            .collect())
    }
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
    /// Refusé : les espaces de travail par écran se règlent dans `monitors.lua`
    /// (`workspace_monitors`). Lu seulement pour le dire clairement.
    #[serde(default)]
    pub workspace: Vec<toml::Value>,
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
            if !p.workspace.is_empty() {
                bail!(
                    "display.toml : profil {} : [[profile.workspace]] n'existe plus ; les espaces de \
                     chaque écran se règlent dans ~/.config/hypr/monitors.lua (workspace_monitors). \
                     Pour ranger des fenêtres, voir [[profile.rescue]]",
                    p.name
                );
            }
            for r in &p.rescue {
                if let Err(e) = r.pairs() {
                    bail!("display.toml : profil {} : {e}", p.name);
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
        assert_eq!(names, ["nomade", "boulot"]);
        let b = c.find("boulot").unwrap();
        assert_eq!(b.monitor.len(), 2);
        assert_eq!(b.monitor[1].transform, Some(1));
        assert_eq!(b.internal.as_ref().unwrap().scale, Some(Scale::Float(1.6)));
        assert_eq!(b.detect_any.len(), 2);
    }

    #[test]
    fn rescue_par_numero_plage_ou_nom() {
        let r = |from: &str, to: &str| {
            Rescue {
                from: from.into(),
                to: to.into(),
            }
            .pairs()
        };
        let p = r("1-3", "21-23").unwrap();
        assert_eq!(p[0], (RescueFrom::Id(1), "21".into()));
        assert_eq!(p[2], (RescueFrom::Id(3), "23".into()));
        assert_eq!(
            r("11-20", "21").unwrap()[9],
            (RescueFrom::Id(20), "21".into())
        );
        assert_eq!(
            r("mid", "11").unwrap(),
            [(RescueFrom::Name("mid".into()), "11".into())]
        );
        assert!(r("1-3", "21-22").is_err());
        assert!(r("mid", "1-2").is_err());
        assert!(r("1", "mid").is_err());
    }

    #[test]
    fn regles_d_espaces_refusees() {
        let e = Config::parse(
            "[[profile]]\nname = \"a\"\n[[profile.workspace]]\nworkspace = \"1-10\"\nmonitor = \"internal\"\n",
        )
        .unwrap_err();
        assert!(format!("{e:#}").contains("monitors.lua"), "{e:#}");
        assert!(Config::parse(
            "[[profile]]\nname = \"a\"\n[[profile.rescue]]\nfrom = \"1-3\"\nto = \"5-6\"\n"
        )
        .is_err());
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
