//! Seuils, dans `~/.config/poste/power.toml` (`XDG_CONFIG_HOME` respecté). Fichier absent :
//! valeurs par défaut. Clés absentes : valeur par défaut de la clé.
//!
//! ```toml
//! interval = 60            # secondes entre deux passes (10 au minimum)
//!
//! [battery]
//! low = 20                 # % : « batterie faible »
//! critical = 10            # % : « batterie critique » (notification urgente)
//! drain_w = 12.0           # W : perte réelle qui compte comme « se vide sur secteur »
//! drain_samples = 2        # passes consécutives avant d'alerter (anti-rebond)
//!
//! [disk]
//! percent = 90             # % d'occupation qui déclenche l'alerte
//! free_gb = 0              # Go libres en dessous desquels alerter (0 : désactivé)
//! paths = ["/", "/home"]   # points de montage ; /home est ignoré s'il partage le disque de /
//! ```

use anyhow::{bail, Context, Result};
use poste_core::Env;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Battery {
    pub low: u8,
    pub critical: u8,
    pub drain_w: f64,
    pub drain_samples: u32,
}

impl Default for Battery {
    fn default() -> Self {
        Self {
            low: 20,
            critical: 10,
            drain_w: 12.0,
            drain_samples: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Disk {
    pub percent: u8,
    pub free_gb: u64,
    pub paths: Vec<String>,
}

impl Default for Disk {
    fn default() -> Self {
        Self {
            percent: 90,
            free_gb: 0,
            paths: vec!["/".into(), "/home".into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct Config {
    pub interval: u64,
    pub battery: Battery,
    pub disk: Disk,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval: 60,
            battery: Battery::default(),
            disk: Disk::default(),
        }
    }
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        let c: Config = toml::from_str(text).context("power.toml invalide")?;
        c.validate()?;
        Ok(c)
    }

    fn validate(&self) -> Result<()> {
        let b = &self.battery;
        if b.critical == 0 || b.critical >= b.low || b.low > 100 {
            bail!("power.toml : il faut 0 < battery.critical < battery.low <= 100");
        }
        if self.disk.percent == 0 || self.disk.percent > 100 {
            bail!("power.toml : disk.percent doit être entre 1 et 100");
        }
        if self.interval < 10 {
            bail!("power.toml : interval doit être d'au moins 10 secondes");
        }
        if b.drain_samples == 0 {
            bail!("power.toml : battery.drain_samples doit être d'au moins 1");
        }
        Ok(())
    }

    pub fn file(env: &Env) -> PathBuf {
        env.config_home.join("poste/power.toml")
    }

    /// Charge le fichier ; absent, les valeurs par défaut.
    pub fn load(env: &Env) -> Result<Self> {
        let f = Self::file(env);
        match std::fs::read_to_string(&f) {
            Ok(t) => Self::parse(&t).with_context(|| f.display().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", f.display())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defauts_et_surcharge_partielle() {
        let c = Config::parse("").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.battery.low, 20);
        assert_eq!(c.disk.percent, 90);
        let c =
            Config::parse("[battery]\nlow = 30\n[disk]\npercent = 80\npaths = [\"/\"]\n").unwrap();
        assert_eq!(c.battery.low, 30);
        assert_eq!(c.battery.critical, 10);
        assert_eq!(c.disk.paths, ["/"]);
    }

    #[test]
    fn valeurs_incoherentes_refusees() {
        assert!(Config::parse("[battery]\nlow = 10\ncritical = 10\n").is_err());
        assert!(Config::parse("[battery]\ncritical = 0\n").is_err());
        assert!(Config::parse("[disk]\npercent = 101\n").is_err());
        assert!(Config::parse("interval = 1\n").is_err());
        assert!(Config::parse("[battery]\nlow = \"x\"\n").is_err());
    }
}
