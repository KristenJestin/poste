//! Lecture de `/sys/class/power_supply/*` (dossier remplaçable par `POSTE_POWER_SUPPLY_DIR`,
//! RÉSERVÉE AUX TESTS : faux arbre en dossier temporaire).
//!
//! Seules les alimentations du SYSTÈME comptent : une pile de périphérique (souris, clavier :
//! `scope` = `Device`, ex. `hidpp_battery_0`) n'est jamais « la batterie » de la machine.

use std::path::{Path, PathBuf};

pub const SUPPLY_DIR_VAR: &str = "POSTE_POWER_SUPPLY_DIR";

pub fn supply_dir() -> PathBuf {
    std::env::var_os(SUPPLY_DIR_VAR)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/sys/class/power_supply"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Charging,
    Discharging,
    /// « Not charging » : branché mais le chargeur ne charge pas (limite de charge, ou chargeur insuffisant).
    NotCharging,
    Full,
    Unknown,
}

impl Status {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "charging" => Self::Charging,
            "discharging" => Self::Discharging,
            "not charging" => Self::NotCharging,
            "full" => Self::Full,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Charging => "en charge",
            Self::Discharging => "se décharge",
            Self::NotCharging => "ne charge pas",
            Self::Full => "pleine",
            Self::Unknown => "état inconnu",
        }
    }

    /// Vrai quand la batterie reçoit de l'énergie (ou est pleine) : pas d'alerte de niveau.
    pub fn is_charging(self) -> bool {
        matches!(self, Self::Charging | Self::Full)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub name: String,
    /// Pourcentage 0 à 100.
    pub level: u8,
    pub status: Status,
    /// Énergie restante en µWh (`energy_now`, ou `charge_now` x `voltage_now`), pour mesurer le débit réel.
    pub energy_uwh: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Supplies {
    pub battery: Option<Battery>,
    pub ac_online: bool,
}

fn read(dir: &Path, file: &str) -> Option<String> {
    std::fs::read_to_string(dir.join(file))
        .ok()
        .map(|s| s.trim().to_string())
}

fn read_u64(dir: &Path, file: &str) -> Option<u64> {
    read(dir, file)?.parse().ok()
}

fn energy_uwh(dir: &Path) -> Option<u64> {
    if let Some(e) = read_u64(dir, "energy_now") {
        return Some(e);
    }
    // µAh x µV = pWh ; / 1e6 pour des µWh.
    let c = read_u64(dir, "charge_now")?;
    let v = read_u64(dir, "voltage_now")?;
    Some((c as u128 * v as u128 / 1_000_000) as u64)
}

/// Lit batterie et secteur. Aucune batterie système : `battery` vaut `None`.
pub fn read_supplies(root: &Path) -> Supplies {
    let mut names: Vec<_> = match std::fs::read_dir(root) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(_) => return Supplies::default(),
    };
    names.sort();
    let mut out = Supplies::default();
    for dir in names {
        if read(&dir, "scope").as_deref() == Some("Device") {
            continue;
        }
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match read(&dir, "type").as_deref() {
            Some("Mains") => {
                if read(&dir, "online").as_deref() == Some("1") {
                    out.ac_online = true;
                }
            }
            Some("Battery") if out.battery.is_none() => {
                let Some(level) = read_u64(&dir, "capacity") else {
                    continue;
                };
                out.battery = Some(Battery {
                    name,
                    level: level.min(100) as u8,
                    status: read(&dir, "status")
                        .map(|s| Status::parse(&s))
                        .unwrap_or(Status::Unknown),
                    energy_uwh: energy_uwh(&dir),
                });
            }
            _ => {}
        }
    }
    out
}

/// Débit réel de décharge en W entre deux mesures (positif : la batterie se vide).
/// `None` si l'intervalle est nul. La mesure d'énergie est la seule vérité : le statut et
/// `power_now` peuvent mentir (chargeur USB-C instable).
pub fn drain_watts(prev_uwh: u64, now_uwh: u64, secs: f64) -> Option<f64> {
    if secs <= 0.0 {
        return None;
    }
    Some((prev_uwh as f64 - now_uwh as f64) / 1e6 * 3600.0 / secs)
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::fs;

    pub fn put(root: &Path, dev: &str, files: &[(&str, &str)]) {
        let d = root.join(dev);
        fs::create_dir_all(&d).unwrap();
        for (k, v) in files {
            fs::write(d.join(k), format!("{v}\n")).unwrap();
        }
    }

    #[test]
    fn batterie_et_secteur() {
        let t = tempfile::tempdir().unwrap();
        put(t.path(), "AC", &[("type", "Mains"), ("online", "1")]);
        put(
            t.path(),
            "BAT0",
            &[
                ("type", "Battery"),
                ("capacity", "42"),
                ("status", "Not charging"),
                ("energy_now", "30000000"),
            ],
        );
        // Pile de souris : ignorée.
        put(
            t.path(),
            "hidpp_battery_0",
            &[
                ("type", "Battery"),
                ("scope", "Device"),
                ("capacity", "5"),
                ("status", "Discharging"),
            ],
        );
        let s = read_supplies(t.path());
        assert!(s.ac_online);
        let b = s.battery.unwrap();
        assert_eq!(b.name, "BAT0");
        assert_eq!(b.level, 42);
        assert_eq!(b.status, Status::NotCharging);
        assert_eq!(b.energy_uwh, Some(30_000_000));
    }

    #[test]
    fn sans_batterie_ni_dossier() {
        let t = tempfile::tempdir().unwrap();
        put(t.path(), "AC", &[("type", "Mains"), ("online", "0")]);
        put(
            t.path(),
            "hidpp_battery_1",
            &[("type", "Battery"), ("scope", "Device"), ("capacity", "50")],
        );
        let s = read_supplies(t.path());
        assert!(s.battery.is_none() && !s.ac_online);
        assert_eq!(read_supplies(&t.path().join("absent")), Supplies::default());
    }

    #[test]
    fn energie_depuis_charge_et_tension() {
        let t = tempfile::tempdir().unwrap();
        put(
            t.path(),
            "BAT1",
            &[
                ("type", "Battery"),
                ("capacity", "100"),
                ("status", "Full"),
                ("charge_now", "4000000"),
                ("voltage_now", "12000000"),
            ],
        );
        let b = read_supplies(t.path()).battery.unwrap();
        assert_eq!(b.energy_uwh, Some(48_000_000));
        assert!(b.status.is_charging());
    }

    #[test]
    fn debit() {
        // 1 Wh perdu en 300 s = 12 W.
        let w = drain_watts(31_000_000, 30_000_000, 300.0).unwrap();
        assert!((w - 12.0).abs() < 1e-9);
        assert!(drain_watts(1, 2, 60.0).unwrap() < 0.0);
        assert!(drain_watts(1, 2, 0.0).is_none());
    }
}
