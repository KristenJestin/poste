//! The real binary with a fake `power_supply` tree, a fake notification sender and a temporary
//! HOME: never the real `/sys`, never a real notification.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_poste-power"))
}

#[test]
fn check_sends_once_then_status() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let run = t.path().join("run");
    let ps = t.path().join("ps");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(ps.join("BAT0")).unwrap();
    for (k, v) in [
        ("type", "Battery"),
        ("capacity", "8"),
        ("status", "Discharging"),
    ] {
        fs::write(ps.join("BAT0").join(k), v).unwrap();
    }
    let log = t.path().join("notif.log");
    let fake = t.path().join("fake-notify");
    fs::write(
        &fake,
        format!("#!/bin/sh\necho \"$@\" >> '{}'\n", log.display()),
    )
    .unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    // Config: no disk, for a deterministic test.
    fs::create_dir_all(home.join(".config/poste")).unwrap();
    fs::write(
        home.join(".config/poste/power.toml"),
        "[disk]\npaths = []\n",
    )
    .unwrap();

    let go = |args: &[&str]| {
        let mut c = bin();
        c.args(args)
            .env("HOME", &home)
            .env_remove("XDG_CONFIG_HOME")
            .env("XDG_RUNTIME_DIR", &run)
            .env("POSTE_POWER_SUPPLY_DIR", &ps)
            .env("POSTE_POWER_NOTIFY", &fake);
        c.output().unwrap()
    };
    assert!(go(&["check"]).status.success());
    assert!(go(&["check"]).status.success());
    let l = fs::read_to_string(&log).unwrap();
    assert_eq!(l.lines().count(), 1, "{l}");
    assert!(l.contains("-u critical") && l.contains("Critical battery"));

    let st = String::from_utf8(go(&["status"]).stdout).unwrap();
    assert!(
        st.contains("8 %") && st.contains("critical: already reported"),
        "{st}"
    );

    assert!(go(&["test"]).status.success());
    assert_eq!(fs::read_to_string(&log).unwrap().lines().count(), 2);
    assert_eq!(go(&["unknown"]).status.code(), Some(2));
}
