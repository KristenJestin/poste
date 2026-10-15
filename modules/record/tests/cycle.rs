//! Full cycle with a fake gpu-screen-recorder: start, pause, resume, toggle, stop,
//! notification, open-last, hotkey. Nothing is recorded; the fake logs its arguments
//! and the signals received.

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
    b.wait_for("the recording state", || b.state_file().exists());
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
            "{cmd} at rest"
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

    // The arguments given to the fake gsr.
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

    // A second start is refused, without touching the recording.
    b.clear_journal();
    let o = b.rust(&["start", "--monitor", "DP-1"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(b.journal().contains("Already recording"));
    assert!(status(&b)["recording"] == true);
    assert!(!b.journal().contains("gpu-screen-recorder"));

    // Pause (SIGUSR2), idempotent, then resume.
    assert_eq!(b.rust(&["pause"]).output().unwrap().status.code(), Some(0));
    b.wait_for("SIGUSR2", || signals(&b) == ["USR2"]);
    let paused = status(&b);
    assert_eq!(paused["paused"], true);
    assert_eq!(b.rust(&["pause"]).output().unwrap().status.code(), Some(0));
    std::thread::sleep(std::time::Duration::from_millis(150));
    assert_eq!(signals(&b), ["USR2"], "a second pause does not signal");
    let frozen = status(&b)["recordMs"].as_i64().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(250));
    assert_eq!(
        status(&b)["recordMs"].as_i64().unwrap(),
        frozen,
        "timer frozen while paused"
    );
    // The menu now only offers Stop / Resume.
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
    // toggle: pause then resume.
    assert_eq!(b.rust(&["toggle"]).output().unwrap().status.code(), Some(0));
    b.wait_for("third SIGUSR2", || signals(&b).len() == 3);
    assert_eq!(status(&b)["paused"], true);
    assert_eq!(b.rust(&["toggle"]).output().unwrap().status.code(), Some(0));
    b.wait_for("fourth SIGUSR2", || signals(&b).len() == 4);
    assert_eq!(status(&b)["paused"], false);

    // watch: a state line in progress.
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

    // open-last during the recording: the file in progress is excluded.
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

    // Stop (SIGINT): the supervisor cleans up and notifies.
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

    // open-last now opens the finished file.
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
fn hotkey_stops_the_recording_in_progress() {
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
    assert!(!j.contains("omarchy-shell"), "the launcher does not open");
}

#[test]
fn chosen_window_writes_pick_json_for_the_picker() {
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
    b.wait_for("the state", || b.state_file().exists());
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
        "window class for lack of sound already emitted: {j}"
    );
    // The picker answers with this window, showing nothing.
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
fn gsr_failure_notifies_the_last_log_line() {
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
fn the_picker_shim_script_goes_through_the_binary() {
    // bin/poste-share-picker (path of xdph.conf) calls ../poste-record share-picker.
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
    // With a fresh pick.json: the binary answers alone.
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
    // Without pick.json: fallback to the usual picker (fake), with the same arguments.
    assert!(c.output().unwrap().status.success());
    assert_eq!(
        b.journal().trim_end(),
        "hyprland-preview-share-picker\t--flag"
    );
    // Without an installed binary: fallback too.
    std::fs::remove_file(module.join("poste-record")).unwrap();
    b.clear_journal();
    assert!(c.output().unwrap().status.success());
    assert_eq!(
        b.journal().trim_end(),
        "hyprland-preview-share-picker\t--flag"
    );
}
