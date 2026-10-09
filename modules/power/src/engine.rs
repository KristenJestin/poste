//! Le cerveau des alertes : fonction pure `step`. Une alerte par passage de seuil ; l'état
//! « déjà alerté » est réarmé quand la condition disparaît. Aucune entrée-sortie ici.

use crate::config::Config;
use crate::disk::Usage;
use crate::supply::Supplies;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Hystérésis du disque : l'occupation doit redescendre de ces points sous le seuil pour réarmer
/// (sinon un disque à 90,0 % oscillerait entre deux alertes).
pub const DISK_HYSTERESIS: f64 = 2.0;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub low: bool,
    #[serde(default)]
    pub critical: bool,
    #[serde(default)]
    pub drain: bool,
    #[serde(default)]
    pub drain_count: u32,
    #[serde(default)]
    pub disks: BTreeSet<String>,
    /// Dernière mesure d'énergie (µWh) et son horodatage (secondes Unix), pour le débit réel.
    #[serde(default)]
    pub last_energy_uwh: Option<u64>,
    #[serde(default)]
    pub last_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Low,
    Critical,
    Drain,
    Disk(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Normal,
    Critical,
}

impl Urgency {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub key: Key,
    pub title: String,
    pub body: String,
    pub urgency: Urgency,
    pub glyph: &'static str,
}

pub const GLYPH_BATTERY: &str = "\u{f140b}";
pub const GLYPH_AC: &str = "\u{f06a5}";
pub const GLYPH_DISK: &str = "\u{f02ca}";

impl State {
    /// Une notification n'a pas pu partir : on réarme pour réessayer à la prochaine passe.
    pub fn unfire(&mut self, key: &Key) {
        match key {
            Key::Low => self.low = false,
            Key::Critical => self.critical = false,
            Key::Drain => self.drain = false,
            Key::Disk(p) => {
                self.disks.remove(p);
            }
        }
    }
}

/// Une passe. `drain_w` : débit réel mesuré depuis la passe précédente (W, positif = se vide).
pub fn step(
    st: &mut State,
    cfg: &Config,
    sup: &Supplies,
    disks: &[Usage],
    drain_w: Option<f64>,
) -> Vec<Alert> {
    let mut out = Vec::new();
    match &sup.battery {
        Some(b) => {
            let cb = &cfg.battery;
            let uncharged = !b.status.is_charging();
            let crit = uncharged && b.level <= cb.critical;
            let low = uncharged && b.level <= cb.low;
            if crit {
                if !st.critical {
                    out.push(Alert {
                        key: Key::Critical,
                        title: "Batterie critique".into(),
                        body: format!("{} % restants : branchez le chargeur maintenant.", b.level),
                        urgency: Urgency::Critical,
                        glyph: GLYPH_BATTERY,
                    });
                }
                // Un saut direct au critique ne déclenche pas aussi « faible ».
                st.critical = true;
                st.low = true;
            } else {
                st.critical = false;
                if low {
                    if !st.low {
                        out.push(Alert {
                            key: Key::Low,
                            title: "Batterie faible".into(),
                            body: format!(
                                "{} % restants : pensez à brancher le chargeur.",
                                b.level
                            ),
                            urgency: Urgency::Normal,
                            glyph: GLYPH_BATTERY,
                        });
                    }
                    st.low = true;
                } else {
                    st.low = false;
                }
            }

            // Branché mais la batterie se vide : le statut peut mentir, le débit mesuré non.
            let draining = sup.ac_online
                && (b.status == crate::supply::Status::Discharging
                    || drain_w.is_some_and(|w| w > cb.drain_w));
            if draining {
                st.drain_count += 1;
                if st.drain_count >= cb.drain_samples && !st.drain {
                    st.drain = true;
                    let rate = drain_w
                        .filter(|w| *w > 0.0)
                        .map(|w| format!(" (environ {w:.0} W)"))
                        .unwrap_or_default();
                    out.push(Alert {
                        key: Key::Drain,
                        title: "Branché, mais la batterie se vide".into(),
                        body: format!(
                            "Batterie à {} %, elle perd de la charge{rate} alors que le secteur est branché : le chargeur ne suit pas. Débranchez et rebranchez le câble.",
                            b.level
                        ),
                        urgency: Urgency::Critical,
                        glyph: GLYPH_AC,
                    });
                }
            } else {
                st.drain_count = 0;
                st.drain = false;
            }
        }
        None => {
            st.low = false;
            st.critical = false;
            st.drain = false;
            st.drain_count = 0;
        }
    }

    let cd = &cfg.disk;
    let paths: BTreeSet<&str> = disks.iter().map(|d| d.path.as_str()).collect();
    st.disks.retain(|p| paths.contains(p.as_str()));
    for d in disks {
        let free_gb = d.free_bytes as f64 / 1e9;
        let over =
            d.used_pct >= f64::from(cd.percent) || (cd.free_gb > 0 && free_gb < cd.free_gb as f64);
        let clear = d.used_pct < f64::from(cd.percent) - DISK_HYSTERESIS
            && (cd.free_gb == 0 || free_gb >= cd.free_gb as f64 + 1.0);
        let known = st.disks.contains(&d.path);
        if over && !known {
            st.disks.insert(d.path.clone());
            out.push(Alert {
                key: Key::Disk(d.path.clone()),
                title: "Disque presque plein".into(),
                body: format!(
                    "{} est plein à {:.0} % ({:.0} Go libres).",
                    d.path, d.used_pct, free_gb
                ),
                urgency: Urgency::Normal,
                glyph: GLYPH_DISK,
            });
        } else if known && clear {
            st.disks.remove(&d.path);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::supply::{Battery, Status};

    fn sup(level: u8, status: Status, ac: bool) -> Supplies {
        Supplies {
            battery: Some(Battery {
                name: "BAT0".into(),
                level,
                status,
                energy_uwh: None,
            }),
            ac_online: ac,
        }
    }

    fn disk(p: &str, pct: f64) -> Usage {
        let mut u = crate::disk::from_blocks(p, 1_000_000_000, 100, 0, 0);
        u.used_pct = pct;
        u.free_bytes = 50_000_000_000;
        u
    }

    fn keys(a: &[Alert]) -> Vec<Key> {
        a.iter().map(|x| x.key.clone()).collect()
    }

    #[test]
    fn faible_puis_critique_une_fois_chacun() {
        let cfg = Config::default();
        let mut st = State::default();
        assert!(step(
            &mut st,
            &cfg,
            &sup(50, Status::Discharging, false),
            &[],
            None
        )
        .is_empty());
        assert_eq!(
            keys(&step(
                &mut st,
                &cfg,
                &sup(20, Status::Discharging, false),
                &[],
                None
            )),
            [Key::Low]
        );
        // Pas de répétition.
        assert!(step(
            &mut st,
            &cfg,
            &sup(19, Status::Discharging, false),
            &[],
            None
        )
        .is_empty());
        let a = step(
            &mut st,
            &cfg,
            &sup(10, Status::Discharging, false),
            &[],
            None,
        );
        assert_eq!(keys(&a), [Key::Critical]);
        assert_eq!(a[0].urgency, Urgency::Critical);
        assert!(step(
            &mut st,
            &cfg,
            &sup(9, Status::Discharging, false),
            &[],
            None
        )
        .is_empty());
    }

    #[test]
    fn saut_direct_au_critique_une_seule_alerte() {
        let cfg = Config::default();
        let mut st = State::default();
        let a = step(
            &mut st,
            &cfg,
            &sup(8, Status::Discharging, false),
            &[],
            None,
        );
        assert_eq!(keys(&a), [Key::Critical]);
    }

    #[test]
    fn rearmement_en_chargeant_ou_au_dessus() {
        let cfg = Config::default();
        let mut st = State::default();
        step(
            &mut st,
            &cfg,
            &sup(15, Status::Discharging, false),
            &[],
            None,
        );
        // On branche : en charge, réarmé.
        assert!(step(&mut st, &cfg, &sup(15, Status::Charging, true), &[], None).is_empty());
        assert_eq!(
            keys(&step(
                &mut st,
                &cfg,
                &sup(15, Status::Discharging, false),
                &[],
                None
            )),
            [Key::Low]
        );
        // Remonte au dessus du seuil : réarmé aussi.
        step(
            &mut st,
            &cfg,
            &sup(60, Status::Discharging, false),
            &[],
            None,
        );
        assert_eq!(
            keys(&step(
                &mut st,
                &cfg,
                &sup(20, Status::Discharging, false),
                &[],
                None
            )),
            [Key::Low]
        );
    }

    #[test]
    fn critique_redescendu_en_faible_ne_realerte_pas_faible() {
        let cfg = Config::default();
        let mut st = State::default();
        step(
            &mut st,
            &cfg,
            &sup(9, Status::Discharging, false),
            &[],
            None,
        );
        // 12 % (en charge lente non signalée) : critique réarmé mais pas de nouvelle alerte « faible ».
        assert!(step(
            &mut st,
            &cfg,
            &sup(12, Status::Discharging, false),
            &[],
            None
        )
        .is_empty());
        assert_eq!(
            keys(&step(
                &mut st,
                &cfg,
                &sup(9, Status::Discharging, false),
                &[],
                None
            )),
            [Key::Critical]
        );
    }

    #[test]
    fn seuils_reglables() {
        let cfg = Config::parse("[battery]\nlow = 35\ncritical = 5\n").unwrap();
        let mut st = State::default();
        assert_eq!(
            keys(&step(
                &mut st,
                &cfg,
                &sup(30, Status::Discharging, false),
                &[],
                None
            )),
            [Key::Low]
        );
    }

    #[test]
    fn branche_mais_se_vide_avec_antirebond() {
        let cfg = Config::default(); // drain_samples = 2
        let mut st = State::default();
        let s = sup(60, Status::NotCharging, true);
        // Un seul tour de décharge mesurée : rien.
        assert!(step(&mut st, &cfg, &s, &[], Some(20.0)).is_empty());
        // Un tour propre remet le compteur à zéro.
        assert!(step(&mut st, &cfg, &s, &[], Some(1.0)).is_empty());
        assert!(step(&mut st, &cfg, &s, &[], Some(20.0)).is_empty());
        let a = step(&mut st, &cfg, &s, &[], Some(20.0));
        assert_eq!(keys(&a), [Key::Drain]);
        assert!(a[0].body.contains("20 W"));
        assert!(step(&mut st, &cfg, &s, &[], Some(20.0)).is_empty());
        // Fin de l'épisode puis nouvel épisode.
        step(&mut st, &cfg, &s, &[], Some(0.0));
        step(&mut st, &cfg, &s, &[], Some(20.0));
        assert_eq!(
            keys(&step(&mut st, &cfg, &s, &[], Some(20.0))),
            [Key::Drain]
        );
    }

    #[test]
    fn statut_discharging_sur_secteur_compte_sans_mesure() {
        let cfg = Config::parse("[battery]\ndrain_samples = 1\n").unwrap();
        let mut st = State::default();
        let a = step(
            &mut st,
            &cfg,
            &sup(60, Status::Discharging, true),
            &[],
            None,
        );
        assert_eq!(keys(&a), [Key::Drain]);
        // Sans secteur : pas de cette alerte.
        let mut st = State::default();
        assert!(step(
            &mut st,
            &cfg,
            &sup(60, Status::Discharging, false),
            &[],
            Some(30.0)
        )
        .is_empty());
    }

    #[test]
    fn sans_batterie_seulement_le_disque() {
        let cfg = Config::default();
        let mut st = State::default();
        let none = Supplies::default();
        assert!(step(&mut st, &cfg, &none, &[disk("/", 50.0)], None).is_empty());
        assert_eq!(
            keys(&step(&mut st, &cfg, &none, &[disk("/", 91.0)], None)),
            [Key::Disk("/".into())]
        );
    }

    #[test]
    fn disque_une_fois_hysteresis_et_rearmement() {
        let cfg = Config::default();
        let mut st = State::default();
        let none = Supplies::default();
        let a = step(
            &mut st,
            &cfg,
            &none,
            &[disk("/", 93.0), disk("/home", 40.0)],
            None,
        );
        assert_eq!(keys(&a), [Key::Disk("/".into())]);
        assert!(a[0].body.contains("93 %"));
        assert!(step(&mut st, &cfg, &none, &[disk("/", 95.0)], None).is_empty());
        // 89 % : sous le seuil mais dans l'hystérésis, toujours armé... donc pas de ré-alerte à 90.
        assert!(step(&mut st, &cfg, &none, &[disk("/", 89.0)], None).is_empty());
        assert!(step(&mut st, &cfg, &none, &[disk("/", 90.5)], None).is_empty());
        // Redescend nettement : réarmé.
        step(&mut st, &cfg, &none, &[disk("/", 80.0)], None);
        assert_eq!(
            keys(&step(&mut st, &cfg, &none, &[disk("/", 92.0)], None)),
            [Key::Disk("/".into())]
        );
    }

    #[test]
    fn alerte_non_envoyee_est_retentee() {
        let cfg = Config::default();
        let mut st = State::default();
        let s = sup(15, Status::Discharging, false);
        let a = step(&mut st, &cfg, &s, &[], None);
        st.unfire(&a[0].key);
        assert_eq!(keys(&step(&mut st, &cfg, &s, &[], None)), [Key::Low]);
    }

    #[test]
    fn seuil_en_go_libres() {
        let cfg = Config::parse("[disk]\nfree_gb = 100\n").unwrap();
        let mut st = State::default();
        let d = disk("/", 10.0); // 50 Go libres < 100
        assert_eq!(
            keys(&step(&mut st, &cfg, &Supplies::default(), &[d], None)),
            [Key::Disk("/".into())]
        );
    }
}
