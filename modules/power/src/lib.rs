//! Module power de poste : alertes batterie et disque, par un démon utilisateur léger.
//! Se lance par `poste power <commande>` (aide : `USAGE`).

pub mod config;
pub mod disk;
pub mod engine;
pub mod notify;
pub mod supply;

use config::Config;
use engine::{Alert, Key, State, Urgency};
use poste_core::Env;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const USAGE: &str = "Module power de poste : alertes batterie et disque.

  poste power status    batterie, secteur, disques, seuils et alertes déjà envoyées
  poste power test      envoie une notification d'exemple
  poste power check     une passe : envoie les alertes dues
  poste power watch     le démon (une passe toutes les 60 s ; unité poste-power.service)

Seuils : ~/.config/poste/power.toml (voir le README du module).";

/// Fichier d'état (alertes déjà envoyées) : `$XDG_RUNTIME_DIR/poste-power/state.json`, perdu au
/// redémarrage de la machine ; sans `XDG_RUNTIME_DIR`, sous le dossier d'état.
pub fn state_file(env: &Env) -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d).join("poste-power/state.json"),
        None => env.state_dir().join("power-state.json"),
    }
}

pub fn load_state(file: &Path) -> State {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_state(file: &Path, st: &State) {
    if let Some(d) = file.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = file.with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_string(st).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, file);
    }
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

/// Une passe complète : lit l'état, calcule les alertes, les envoie avec `send`, note l'état.
/// Renvoie les alertes envoyées avec succès.
pub fn pass(
    cfg: &Config,
    supply_root: &Path,
    state_path: &Path,
    now: f64,
    send: &dyn Fn(&Alert) -> bool,
) -> Vec<Alert> {
    let mut st = load_state(state_path);
    let sup = supply::read_supplies(supply_root);

    // Débit réel depuis la passe précédente ; ignoré après une longue veille (mesure sans sens).
    let mut drain = None;
    let energy = sup.battery.as_ref().and_then(|b| b.energy_uwh);
    if let (Some(prev), Some(at), Some(cur)) = (st.last_energy_uwh, st.last_at, energy) {
        let dt = now - at;
        if dt > 0.0 && dt <= (cfg.interval * 3) as f64 {
            drain = supply::drain_watts(prev, cur, dt);
        }
    }
    st.last_energy_uwh = energy;
    st.last_at = energy.map(|_| now);

    let disks = disk::usages(&cfg.disk.paths);
    let alerts = engine::step(&mut st, cfg, &sup, &disks, drain);
    let mut sent = Vec::new();
    for a in alerts {
        if send(&a) {
            sent.push(a);
        } else {
            st.unfire(&a.key);
        }
    }
    // L'énergie change à chaque passe : on réécrit à chaque fois (petit fichier dans /run).
    save_state(state_path, &st);
    sent
}

fn cmd_status(env: &Env, cfg: &Config) -> i32 {
    let sup = supply::read_supplies(&supply::supply_dir());
    let st = load_state(&state_file(env));
    println!("Alimentation");
    match &sup.battery {
        Some(b) => {
            println!(
                "  batterie   {} : {} % ({})",
                b.name,
                b.level,
                b.status.label()
            );
            println!(
                "  secteur    {}",
                if sup.ac_online {
                    "branché"
                } else {
                    "non branché"
                }
            );
            let c = &cfg.battery;
            println!(
                "  seuils     faible {} %, critique {} %, perte sur secteur > {} W pendant {} passe(s)",
                c.low, c.critical, c.drain_w, c.drain_samples
            );
            let flag = |b: bool| if b { "déjà signalé" } else { "non" };
            println!(
                "  alertes    faible : {}, critique : {}, perte sur secteur : {}",
                flag(st.low),
                flag(st.critical),
                flag(st.drain)
            );
        }
        None => println!("  pas de batterie : seule l'alerte disque est active"),
    }
    println!("Disques (alerte à {} % d'occupation)", cfg.disk.percent);
    let disks = disk::usages(&cfg.disk.paths);
    if disks.is_empty() {
        println!("  aucun chemin lisible parmi {:?}", cfg.disk.paths);
    }
    for d in disks {
        println!(
            "  {:<8} {:.0} % occupé, {:.0} Go libres sur {:.0} Go{}",
            d.path,
            d.used_pct,
            d.free_bytes as f64 / 1e9,
            d.total_bytes as f64 / 1e9,
            if st.disks.contains(&d.path) {
                "  [déjà signalé]"
            } else {
                ""
            }
        );
    }
    println!(
        "Passe toutes les {} s ; réglages : {}",
        cfg.interval,
        env.display(&Config::file(env))
    );
    0
}

fn cmd_test() -> i32 {
    let a = Alert {
        key: Key::Low,
        title: "Test des alertes".into(),
        body: "Ceci est une notification d'exemple de poste power.".into(),
        urgency: Urgency::Normal,
        glyph: engine::GLYPH_BATTERY,
    };
    if notify::send(&a) {
        println!("Notification envoyée.");
        0
    } else {
        eprintln!(
            "poste power : la notification n'a pas pu être envoyée (omarchy-notification-send)"
        );
        1
    }
}

pub fn run(argv: &[String]) -> i32 {
    let Some(cmd) = argv.first() else {
        println!("{USAGE}");
        return 2;
    };
    let env = match Env::from_process() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poste power : {e:#}");
            return 1;
        }
    };
    let cfg = match Config::load(&env) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("poste power : {e:#}");
            return 1;
        }
    };
    match cmd.as_str() {
        "status" => cmd_status(&env, &cfg),
        "test" => cmd_test(),
        "check" => {
            let sent = pass(
                &cfg,
                &supply::supply_dir(),
                &state_file(&env),
                now_secs(),
                &notify::send,
            );
            println!("{} alerte(s) envoyée(s).", sent.len());
            0
        }
        "watch" => loop {
            // Rechargée à chaque passe : modifier power.toml n'exige pas de redémarrage.
            let cfg = Config::load(&env).unwrap_or_else(|e| {
                eprintln!("poste power : {e:#} (réglages précédents gardés)");
                cfg.clone()
            });
            pass(
                &cfg,
                &supply::supply_dir(),
                &state_file(&env),
                now_secs(),
                &notify::send,
            );
            std::thread::sleep(Duration::from_secs(cfg.interval));
        },
        _ => {
            println!("{USAGE}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use supply::tests::put;

    #[test]
    fn passes_successives_sans_repetition_et_debit_mesure() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("ps");
        put(&root, "AC", &[("type", "Mains"), ("online", "1")]);
        let bat = |lvl: &str, e: &str| {
            put(
                &root,
                "BAT0",
                &[
                    ("type", "Battery"),
                    ("capacity", lvl),
                    ("status", "Not charging"),
                    ("energy_now", e),
                ],
            )
        };
        let state = t.path().join("run/state.json");
        let mut cfg = Config::default();
        cfg.disk.paths = vec![];
        let log = RefCell::new(Vec::new());
        let send = |a: &Alert| {
            log.borrow_mut().push(a.key.clone());
            true
        };
        bat("60", "40000000");
        assert!(pass(&cfg, &root, &state, 1000.0, &send).is_empty());
        // 1 Wh perdu en 60 s = 60 W : 1re passe de perte, 2e passe : alerte.
        bat("59", "39000000");
        assert!(pass(&cfg, &root, &state, 1060.0, &send).is_empty());
        bat("58", "38000000");
        assert_eq!(pass(&cfg, &root, &state, 1120.0, &send).len(), 1);
        bat("57", "37000000");
        assert!(pass(&cfg, &root, &state, 1180.0, &send).is_empty());
        assert_eq!(*log.borrow(), [Key::Drain]);
        // L'état survit (nouveau « processus »), une longue veille ignore la mesure.
        bat("30", "10000000");
        assert!(pass(&cfg, &root, &state, 99999.0, &send).is_empty());
    }

    #[test]
    fn echec_d_envoi_retente() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("ps");
        put(
            &root,
            "BAT0",
            &[
                ("type", "Battery"),
                ("capacity", "15"),
                ("status", "Discharging"),
            ],
        );
        let state = t.path().join("state.json");
        let mut cfg = Config::default();
        cfg.disk.paths = vec![];
        assert!(pass(&cfg, &root, &state, 0.0, &|_| false).is_empty());
        assert_eq!(pass(&cfg, &root, &state, 60.0, &|_| true).len(), 1);
        assert!(pass(&cfg, &root, &state, 120.0, &|_| true).is_empty());
    }

    #[test]
    fn arguments_de_notification() {
        let a = Alert {
            key: Key::Critical,
            title: "T".into(),
            body: "B".into(),
            urgency: Urgency::Critical,
            glyph: engine::GLYPH_BATTERY,
        };
        let v = notify::args(&a);
        assert_eq!(&v[v.len() - 4..], ["-u", "critical", "T", "B"]);
        assert!(v.contains(&"--app-name".to_string()));
    }
}
