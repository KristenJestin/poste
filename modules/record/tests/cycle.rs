//! Cycle complet avec un faux gpu-screen-recorder : start, pause, resume, toggle, stop,
//! notification, open-last, hotkey. Rien n'est enregistré ; le faux journalise ses arguments
//! et les signaux reçus.

mod common;
use common::*;
use serde_json::Value;
use std::io::{BufRead, BufReader};

fn status(b: &Bench) -> Value {
    let o = b.rust(&["status"]).output().unwrap();
    assert!(o.status.success());
    serde_json::from_str(&text(&o)).unwrap()
}

fn signals(b: &Bench) -> Vec<String> {
    b.journal()
        .lines()
        .filter_map(|l| l.strip_prefix("gsr-signal\t").map(String::from))
        .collect()
}

fn start_monitor(b: &Bench) {
    let o = b
        .rust(&["start", "--monitor", "DP-1", "--audio", "desktop"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    b.wait_for("l'état d'enregistrement", || b.state_file().exists());
    b.wait_gsr_ready();
}

#[test]
fn cycle_start_pause_resume_stop_open_last() {
    let b = Bench::new();
    b.fake("gsr_mode", "hold");
    b.presets(r#"{"defaults": {"dir": "~/Videos/Rec"}}"#);
    std::fs::create_dir_all(b.videos.join("Rec")).unwrap();

    // Rien en cours.
    assert_eq!(
        status(&b),
        serde_json::json!({"t": "status", "recording": false})
    );
    for cmd in ["stop", "pause", "resume", "toggle"] {
        assert_eq!(
            b.rust(&[cmd]).output().unwrap().status.code(),
            Some(1),
            "{cmd} au repos"
        );
    }

    start_monitor(&b);
    let st = status(&b);
    assert_eq!(st["recording"], true);
    assert_eq!(st["paused"], false);
    assert_eq!(st["label"], "Screen DP-1");
    let file = st["file"].as_str().unwrap().to_string();
    assert!(file.contains("/Videos/Rec/Screen DP-1 "), "{file}");
    assert!(file.ends_with(".mp4"));

    // Les arguments donnés au faux gsr.
    let j = b.journal();
    let line = j
        .lines()
        .find(|l| l.starts_with("gpu-screen-recorder\t-w"))
        .unwrap();
    let args: Vec<&str> = line.split('\t').skip(1).collect();
    assert_eq!(
        args[..args.len() - 5],
        [
            "-w",
            "DP-1",
            "-f",
            "60",
            "-q",
            "very_high",
            "-k",
            "auto",
            "-fm",
            "cfr",
            "-cursor",
            "yes",
            "-fallback-cpu-encoding",
            "yes",
            "-o"
        ][..]
    );
    assert_eq!(
        args[args.len() - 5..],
        [file.as_str(), "-ac", "aac", "-a", "default_output"]
    );

    // Un second start est refusé, sans toucher à l'enregistrement.
    b.clear_journal();
    let o = b.rust(&["start", "--monitor", "DP-1"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(b.journal().contains("Already recording"));
    assert!(status(&b)["recording"] == true);
    assert!(!b.journal().contains("gpu-screen-recorder"));

    // Pause (SIGUSR2), idempotente, puis reprise.
    assert_eq!(b.rust(&["pause"]).output().unwrap().status.code(), Some(0));
    b.wait_for("SIGUSR2", || signals(&b) == ["USR2"]);
    let paused = status(&b);
    assert_eq!(paused["paused"], true);
    assert_eq!(b.rust(&["pause"]).output().unwrap().status.code(), Some(0));
    std::thread::sleep(std::time::Duration::from_millis(150));
    assert_eq!(signals(&b), ["USR2"], "une seconde pause ne signale pas");
    let frozen = status(&b)["recordMs"].as_i64().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert_eq!(
        status(&b)["recordMs"].as_i64().unwrap(),
        frozen,
        "chrono figé en pause"
    );
    // Le menu ne propose plus que Stop / Resume.
    let items: Value = serde_json::from_str(&text(&b.rust(&["items"]).output().unwrap())).unwrap();
    let ids: Vec<&str> = items
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["stop", "pause"]);
    assert_eq!(items[1]["name"], "Resume recording");

    assert_eq!(b.rust(&["resume"]).output().unwrap().status.code(), Some(0));
    b.wait_for("second SIGUSR2", || signals(&b).len() == 2);
    assert_eq!(status(&b)["paused"], false);
    assert!(status(&b)["recordMs"].as_i64().unwrap() >= frozen);
    // toggle : pause puis reprise.
    assert_eq!(b.rust(&["toggle"]).output().unwrap().status.code(), Some(0));
    b.wait_for("troisième SIGUSR2", || signals(&b).len() == 3);
    assert_eq!(status(&b)["paused"], true);
    assert_eq!(b.rust(&["toggle"]).output().unwrap().status.code(), Some(0));
    b.wait_for("quatrième SIGUSR2", || signals(&b).len() == 4);
    assert_eq!(status(&b)["paused"], false);

    // watch : une ligne d'état en cours.
    let mut w = b
        .rust(&["watch"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut first = String::new();
    BufReader::new(w.stdout.take().unwrap())
        .read_line(&mut first)
        .unwrap();
    let _ = w.kill();
    let _ = w.wait();
    let v: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(v["t"], "status");
    assert_eq!(v["recording"], true);

    // open-last pendant l'enregistrement : le fichier en cours est exclu.
    b.clear_journal();
    assert_eq!(
        b.rust(&["open-last"]).output().unwrap().status.code(),
        Some(1)
    );
    assert!(
        b.journal().contains("No recording yet\t~/Videos/Rec"),
        "{}",
        b.journal()
    );

    // Stop (SIGINT) : le superviseur nettoie et notifie.
    b.clear_journal();
    assert_eq!(b.rust(&["stop"]).output().unwrap().status.code(), Some(0));
    b.wait_notification();
    let j = b.journal();
    assert!(j.contains("gsr-signal\tINT"));
    let name = file.rsplit('/').next().unwrap();
    let dir = &file[..file.len() - name.len() - 1];
    let note = j
        .lines()
        .find(|l| l.starts_with("omarchy-notification-send\t"))
        .unwrap();
    let parts: Vec<&str> = note.split('\t').collect();
    assert_eq!(
        parts[1..],
        [
            "--app-name",
            "Recorder",
            "-g",
            "\u{f044b}",
            "-u",
            "normal",
            "Recording saved",
            name,
            "--exec",
            "xdg-open",
            dir
        ]
    );
    assert_eq!(
        status(&b),
        serde_json::json!({"t": "status", "recording": false})
    );
    assert!(!b.run.join("poste-record/pick.json").exists());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "video");

    // open-last ouvre maintenant le fichier terminé.
    b.clear_journal();
    assert_eq!(
        b.rust(&["open-last"]).output().unwrap().status.code(),
        Some(0)
    );
    b.wait_for("xdg-open", || b.journal().contains("xdg-open"));
    assert_eq!(b.journal().trim_end(), format!("xdg-open\t{file}"));
    b.clear_journal();
    assert_eq!(
        b.rust(&["open-dir"]).output().unwrap().status.code(),
        Some(0)
    );
    b.wait_for("xdg-open", || b.journal().contains("xdg-open"));
    assert_eq!(b.journal().trim_end(), format!("xdg-open\t{dir}"));
}

#[test]
fn hotkey_arrete_l_enregistrement_en_cours() {
    let b = Bench::new();
    b.fake("gsr_mode", "hold");
    start_monitor(&b);
    b.clear_journal();
    assert_eq!(b.rust(&["hotkey"]).output().unwrap().status.code(), Some(0));
    b.wait_notification();
    let j = b.journal();
    assert!(
        j.contains("gsr-signal\tINT") && j.contains("Recording saved"),
        "{j}"
    );
    assert!(!j.contains("omarchy-shell"), "le lanceur ne s'ouvre pas");
}

#[test]
fn fenetre_choisie_ecrit_pick_json_pour_le_selecteur() {
    let b = Bench::new();
    b.fake("gsr_mode", "hold");
    b.fake(
        "clients.json",
        r#"[{"address": "0xabc", "mapped": true, "title": "T", "class": "Unity", "at": [0,0], "size": [10, 10], "workspace": {"id": 1}}]"#,
    );
    let o = b
        .rust(&["start", "--window", "0xabc", "--audio", "app"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    b.wait_for("l'état", || b.state_file().exists());
    b.wait_gsr_ready();
    let pick: Value = serde_json::from_str(
        &std::fs::read_to_string(b.run.join("poste-record/pick.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(pick["address"], "0xabc");
    assert!(pick["at"].as_f64().unwrap() > 1.0e9);
    let j = b.journal();
    assert!(j.contains("-w\tportal\t-restore-portal-session\tno"));
    assert!(
        j.contains("-a\tapp:Unity"),
        "classe de la fenêtre faute de son déjà émis : {j}"
    );
    // Le sélecteur répond avec cette fenêtre, sans rien afficher.
    let mut c = b.rust(&["share-picker"]);
    c.env("XDPH_WINDOW_SHARING_LIST", "7[HC>]Unity[HT>]T[HE>]abc[HA>]");
    let o = c.output().unwrap();
    assert_eq!(text(&o), "[SELECTION]r/window:7\n");
    b.clear_journal();
    assert_eq!(b.rust(&["stop"]).output().unwrap().status.code(), Some(0));
    b.wait_notification();
    assert!(!b.run.join("poste-record/pick.json").exists());
}

#[test]
fn echec_de_gsr_notifie_la_derniere_ligne_du_journal() {
    let b = Bench::new();
    b.fake("gsr_mode", "fail");
    let o = b.rust(&["start", "--monitor", "DP-1"]).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    b.wait_notification();
    let j = b.journal();
    let note = j
        .lines()
        .find(|l| l.starts_with("omarchy-notification-send\t"))
        .unwrap();
    assert!(
        note.ends_with("-u\tcritical\tRecording failed\tboom: cannot capture"),
        "{note}"
    );
    assert_eq!(
        std::fs::read_to_string(b.run.join("poste-record/last.log")).unwrap(),
        "first line\nboom: cannot capture\n"
    );
}

#[test]
fn le_script_shim_du_selecteur_passe_par_le_binaire() {
    // bin/poste-share-picker (chemin de xdph.conf) appelle ../poste-record share-picker.
    let b = Bench::new();
    let module = b.tmp.path().join("modules/record");
    std::fs::create_dir_all(module.join("bin")).unwrap();
    std::fs::copy(
        manifest_dir().join("bin/poste-share-picker"),
        module.join("bin/poste-share-picker"),
    )
    .unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_poste-record"),
        module.join("poste-record"),
    )
    .unwrap();
    let mut c = std::process::Command::new("sh");
    c.arg(module.join("bin/poste-share-picker"))
        .arg("--flag")
        .env_clear()
        .env("XDG_RUNTIME_DIR", &b.run)
        .env("FAKE_DIR", &b.work)
        .env("XDPH_WINDOW_SHARING_LIST", "7[HC>]Unity[HT>]T[HE>]abc[HA>]")
        .env("PATH", format!("{}:/usr/bin:/bin", b.fakes.display()));
    // Avec un pick.json frais : le binaire répond seul.
    std::fs::create_dir_all(b.run.join("poste-record")).unwrap();
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    std::fs::write(
        b.run.join("poste-record/pick.json"),
        format!(r#"{{"address": "0xabc", "at": {at}}}"#),
    )
    .unwrap();
    assert_eq!(text(&c.output().unwrap()), "[SELECTION]r/window:7\n");
    assert_eq!(b.journal(), "");
    // Sans pick.json : repli sur le sélecteur habituel (faux), avec les mêmes arguments.
    assert!(c.output().unwrap().status.success());
    assert_eq!(
        b.journal().trim_end(),
        "hyprland-preview-share-picker\t--flag"
    );
    // Sans binaire installé : repli aussi.
    std::fs::remove_file(module.join("poste-record")).unwrap();
    b.clear_journal();
    assert!(c.output().unwrap().status.success());
    assert_eq!(
        b.journal().trim_end(),
        "hyprland-preview-share-picker\t--flag"
    );
}
