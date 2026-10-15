//! Power module of poste: battery and disk alerts, through a light user daemon.
//! Run with `poste power <command>` (help: `USAGE`).

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

pub const USAGE: &str = "Power module of poste: battery and disk alerts.

    poste power status    battery, mains, disks, thresholds and alerts already sent
    poste power test      sends a sample notification
    poste power check     one pass: sends the alerts that are due
    poste power watch     the daemon (one pass every 60 s; unit poste-power.service)

Thresholds: ~/.config/poste/power.toml (see the module's README).";

/// State file (alerts already sent): `$XDG_RUNTIME_DIR/poste-power/state.json`, lost at the
/// machine's restart; without `XDG_RUNTIME_DIR`, under the state folder.
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

/// A complete pass: reads the state, computes the alerts, sends them with `send`, notes the state.
/// Returns the alerts sent successfully.
pub fn pass(
    cfg: &Config,
    supply_root: &Path,
    state_path: &Path,
    now: f64,
    send: &dyn Fn(&Alert) -> bool,
) -> Vec<Alert> {
    let mut st = load_state(state_path);
    let sup = supply::read_supplies(supply_root);

    // Real rate since the previous pass; ignored after a long sleep (meaningless measure).
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
    // The energy changes at every pass: we write every time (small file in /run).
    save_state(state_path, &st);
    sent
}

fn cmd_status(env: &Env, cfg: &Config) -> i32 {
    let sup = supply::read_supplies(&supply::supply_dir());
    let st = load_state(&state_file(env));
    println!("Power supply");
    match &sup.battery {
        Some(b) => {
            println!(
                "  battery    {}: {} % ({})",
                b.name,
                b.level,
                b.status.label()
            );
            println!(
                "  mains      {}",
                if sup.ac_online {
                    "plugged in"
                } else {
                    "not plugged in"
                }
            );
            let c = &cfg.battery;
            println!(
                "  thresholds low {} %, critical {} %, loss on mains > {} W for {} pass(es)",
                c.low, c.critical, c.drain_w, c.drain_samples
            );
            let flag = |b: bool| if b { "already reported" } else { "no" };
            println!(
                "  alerts     low: {}, critical: {}, loss on mains: {}",
                flag(st.low),
                flag(st.critical),
                flag(st.drain)
            );
        }
        None => println!("  no battery: only the disk alert is active"),
    }
    println!("Disks (alert at {} % usage)", cfg.disk.percent);
    let disks = disk::usages(&cfg.disk.paths);
    if disks.is_empty() {
        println!("  no readable path among {:?}", cfg.disk.paths);
    }
    for d in disks {
        println!(
            "  {:<8} {:.0} % used, {:.0} GB free of {:.0} GB{}",
            d.path,
            d.used_pct,
            d.free_bytes as f64 / 1e9,
            d.total_bytes as f64 / 1e9,
            if st.disks.contains(&d.path) {
                "  [already reported]"
            } else {
                ""
            }
        );
    }
    println!(
        "Pass every {} s; settings: {}",
        cfg.interval,
        env.display(&Config::file(env))
    );
    0
}

fn cmd_test() -> i32 {
    let a = Alert {
        key: Key::Low,
        title: "Alert test".into(),
        body: "This is a sample notification from poste power.".into(),
        urgency: Urgency::Normal,
        glyph: engine::GLYPH_BATTERY,
    };
    if notify::send(&a) {
        println!("Notification sent.");
        0
    } else {
        eprintln!("poste power: the notification could not be sent (omarchy-notification-send)");
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
            eprintln!("poste power: {e:#}");
            return 1;
        }
    };
    let cfg = match Config::load(&env) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("poste power: {e:#}");
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
            println!("{} alert(s) sent.", sent.len());
            0
        }
        "watch" => loop {
            // Reloaded at each pass: editing power.toml does not require a restart.
            let cfg = Config::load(&env).unwrap_or_else(|e| {
                eprintln!("poste power: {e:#} (previous settings kept)");
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
    fn successive_passes_without_repetition_and_measured_rate() {
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
        // 1 Wh lost in 60 s = 60 W: 1st pass of loss, 2nd pass: alert.
        bat("59", "39000000");
        assert!(pass(&cfg, &root, &state, 1060.0, &send).is_empty());
        bat("58", "38000000");
        assert_eq!(pass(&cfg, &root, &state, 1120.0, &send).len(), 1);
        bat("57", "37000000");
        assert!(pass(&cfg, &root, &state, 1180.0, &send).is_empty());
        assert_eq!(*log.borrow(), [Key::Drain]);
        // The state survives (new "process"), a long sleep ignores the measure.
        bat("30", "10000000");
        assert!(pass(&cfg, &root, &state, 99999.0, &send).is_empty());
    }

    #[test]
    fn send_failure_retried() {
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
    fn notification_arguments() {
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
