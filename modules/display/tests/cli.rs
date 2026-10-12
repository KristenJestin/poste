//! Le vrai binaire avec un faux `hyprctl`, un faux envoi de notification, un faux socket
//! d'événements et un HOME temporaire. Rien n'est appliqué à la vraie session.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Bench {
    _t: tempfile::TempDir,
    home: PathBuf,
    run: PathBuf,
    mons: PathBuf,
    log: PathBuf,
    notif: PathBuf,
    hyprctl: PathBuf,
    notify: PathBuf,
}

fn monitors(list: &[(&str, &str)]) -> String {
    let v: Vec<String> = list
        .iter()
        .map(|(n, d)| format!("{{\"name\":\"{n}\",\"description\":\"{d}\",\"disabled\":false}}"))
        .collect();
    format!("[{}]", v.join(","))
}

fn script(p: &Path, body: &str) {
    fs::write(p, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(p, fs::Permissions::from_mode(0o755)).unwrap();
}

impl Bench {
    fn new() -> Self {
        let t = tempfile::tempdir().unwrap();
        let d = t.path().to_path_buf();
        let b = Bench {
            home: d.join("home"),
            run: d.join("run"),
            mons: d.join("mons.json"),
            log: d.join("hypr.log"),
            notif: d.join("notif.log"),
            hyprctl: d.join("fake-hyprctl"),
            notify: d.join("fake-notify"),
            _t: t,
        };
        fs::create_dir_all(b.home.join(".config/poste")).unwrap();
        fs::create_dir_all(&b.run).unwrap();
        fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/display.toml.example"),
            b.home.join(".config/poste/display.toml"),
        )
        .unwrap();
        script(
            &b.hyprctl,
            &format!(
                "case \"$1\" in\n monitors) cat '{m}' ;;\n clients) echo '[]' ;;\n eval) echo \"$2\" >> '{l}'; echo ok ;;\n *) echo \"$@\" >> '{l}'; echo ok ;;\nesac",
                m = b.mons.display(),
                l = b.log.display()
            ),
        );
        script(
            &b.notify,
            &format!("echo \"$@\" >> '{}'", b.notif.display()),
        );
        b
    }
    fn set(&self, list: &[(&str, &str)]) {
        fs::write(&self.mons, monitors(list)).unwrap();
    }
    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_poste-display"));
        c.args(args)
            .env("HOME", &self.home)
            .env_remove("XDG_CONFIG_HOME")
            .env("XDG_RUNTIME_DIR", &self.run)
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
            .env("POSTE_DISPLAY_HYPRCTL", &self.hyprctl)
            .env("POSTE_DISPLAY_NOTIFY", &self.notify);
        c
    }
    fn go(&self, args: &[&str]) -> (i32, String) {
        let o = self.cmd(args).output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).into_owned(),
        )
    }
    fn evals(&self) -> usize {
        fs::read_to_string(&self.log).map_or(0, |s| s.lines().count())
    }
}

const EDP: (&str, &str) = ("eDP-1", "BOE 0x0B8E");
const AOC_MID: (&str, &str) = ("HDMI-A-1", "AOC 24G2W1G3- SERIE000000002");
const AOC_LEFT: (&str, &str) = ("DP-1", "AOC 24G2W1G3- SERIE000000001");

#[test]
fn auto_applique_une_fois_notifie_et_respecte_le_manuel() {
    let b = Bench::new();
    b.set(&[EDP]);
    let (c, out) = b.go(&["auto"]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("Nomade"));
    let n = b.evals();
    assert!(n > 0);
    assert!(fs::read_to_string(&b.log)
        .unwrap()
        .contains("hl.monitor({ output = \"eDP-1\""));
    let notif = fs::read_to_string(&b.notif).unwrap();
    assert!(
        notif.contains("-u low") && notif.contains("Nomade"),
        "{notif}"
    );
    // Mêmes écrans : rien de plus.
    b.go(&["auto"]);
    assert_eq!(b.evals(), n);
    assert_eq!(fs::read_to_string(&b.notif).unwrap().lines().count(), 1);

    // On branche le bureau : nouveau profil, nouvelle notification.
    b.set(&[EDP, AOC_MID, AOC_LEFT]);
    let (_, out) = b.go(&["auto"]);
    assert!(out.contains("Boulot"), "{out}");
    assert_eq!(fs::read_to_string(&b.notif).unwrap().lines().count(), 2);

    // À la main : nomade tout en restant branché ; auto ne le défait pas.
    let n = b.evals();
    let (c, _) = b.go(&["apply", "nomade"]);
    assert_eq!(c, 0);
    assert!(b.evals() > n);
    let n = b.evals();
    b.go(&["auto"]);
    assert_eq!(b.evals(), n);
    let (_, st) = b.go(&["status"]);
    assert!(
        st.contains("Profil courant : nomade") && st.contains("à la main") && st.contains("boulot"),
        "{st}"
    );

    // Le lanceur (« réappliquer », auto --force) choisit de nouveau selon les écrans.
    let (_, it) = b.go(&["items"]);
    assert!(it.contains("Profil actuel : Nomade"), "{it}");
    let (c, out) = b.go(&["auto", "--force"]);
    assert_eq!(c, 0);
    assert!(out.contains("Boulot"), "{out}");
    let (_, st) = b.go(&["status"]);
    assert!(
        st.contains("Profil courant : boulot") && !st.contains("à la main"),
        "{st}"
    );
}

#[test]
fn dry_run_n_applique_rien_et_erreurs() {
    let b = Bench::new();
    b.set(&[EDP, AOC_MID]);
    let (c, out) = b.go(&["apply", "boulot", "--dry-run"]);
    assert_eq!(c, 0);
    assert!(
        out.contains("hyprctl eval 'hl.monitor({ output = \"HDMI-A-1\"")
            && out.contains("repli : hyprctl keyword monitor"),
        "{out}"
    );
    assert_eq!(b.evals(), 0);
    assert!(
        !b.run.join("poste-display").exists(),
        "pas d'état en simulation"
    );
    let (c, _) = b.go(&["auto", "--dry-run"]);
    assert_eq!(c, 0);
    assert_eq!(b.evals(), 0);
    assert_eq!(b.go(&["apply", "inconnu"]).0, 1);
    assert_eq!(b.go(&["apply"]).0, 2);
    let (_, l) = b.go(&["list"]);
    assert!(l.contains("nomade") && l.contains("boulot"));
    let (_, it) = b.go(&["items"]);
    assert!(it.contains("poste display auto --force"), "{it}");
}

#[test]
fn repli_historique_quand_eval_refuse() {
    let b = Bench::new();
    b.set(&[EDP]);
    script(
        &b.hyprctl,
        &format!(
            "case \"$1\" in\n monitors) cat '{m}' ;;\n clients) echo '[]' ;;\n eval) echo 'error'; exit 1 ;;\n *) echo \"$@\" >> '{l}'; echo ok ;;\nesac",
            m = b.mons.display(),
            l = b.log.display()
        ),
    );
    let (c, out) = b.go(&["apply", "nomade"]);
    assert_eq!(c, 0, "{out}");
    let log = fs::read_to_string(&b.log).unwrap();
    assert!(
        log.contains("keyword monitor eDP-1,2880x1800@120,0x0,1.6"),
        "{log}"
    );
}

#[test]
fn demon_reagit_au_branchement() {
    let b = Bench::new();
    b.set(&[EDP]);
    let sig = "testsig";
    let dir = b.run.join("hypr").join(sig);
    fs::create_dir_all(&dir).unwrap();
    let listener = UnixListener::bind(dir.join(".socket2.sock")).unwrap();
    let mut child = b
        .cmd(&["watch"])
        .env("HYPRLAND_INSTANCE_SIGNATURE", sig)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let (mut sock, _) = listener.accept().unwrap();
    let wait = |what: &str, f: &dyn Fn() -> bool| {
        let end = Instant::now() + Duration::from_secs(10);
        while !f() {
            assert!(Instant::now() < end, "délai dépassé : {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    // Au démarrage : « auto » applique nomade.
    wait("profil initial", &|| b.notif.exists());
    // On branche des écrans : l'événement déclenche le profil boulot (après stabilisation).
    b.set(&[EDP, AOC_MID]);
    sock.write_all(b"monitoradded>>HDMI-A-1\nmonitoraddedv2>>1,HDMI-A-1,AOC\n")
        .unwrap();
    wait("profil boulot", &|| {
        fs::read_to_string(&b.notif).is_ok_and(|s| s.lines().count() >= 2)
    });
    let notif = fs::read_to_string(&b.notif).unwrap();
    assert!(notif.contains("Boulot"), "{notif}");
    assert_eq!(
        notif.lines().count(),
        2,
        "deux événements, une seule application"
    );
    let _ = child.kill();
    let _ = child.wait();
}
