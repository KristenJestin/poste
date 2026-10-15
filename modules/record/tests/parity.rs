//! Parity with the original Python script (`poste-record.py`): the same scenarios are played
//! by both programs, with the same fake outside programs, and the outputs, the
//! gpu-screen-recorder arguments and the notifications are compared as they are.
//! Without `python3`, these tests are skipped.

mod common;
use common::*;

const MONITORS: &str = r#"[
 {"name": "DP-1", "description": "Dell Inc. DELL U2723QE ABCDEF very long description to cut", "width": 3840, "height": 2160, "x": 0, "y": 0, "scale": 1.5, "transform": 0, "activeWorkspace": {"id": 1, "name": "1"}, "specialWorkspace": {"id": 0, "name": ""}},
 {"name": "HDMI-A-1", "description": "Rotated", "width": 1080, "height": 1920, "x": 2560, "y": -200, "scale": 1.0, "transform": 1, "activeWorkspace": {"id": 4, "name": "4"}, "specialWorkspace": {"id": -98, "name": "special:magic"}},
 {"name": "VIRT-1", "description": "", "width": 1001, "height": 5, "x": 0, "y": 0, "scale": 2.0, "transform": 3, "activeWorkspace": {"id": 9, "name": "9"}, "specialWorkspace": {"id": 0, "name": ""}}
]"#;

const CLIENTS: &str = r#"[
 {"address": "0x5581a", "mapped": true, "title": "Project - Unity 6 - Scene", "class": "Unity", "initialClass": "Unity", "at": [10, 20], "size": [1900, 1000], "workspace": {"id": 1, "name": "1"}},
 {"address": "0x5581b", "mapped": true, "title": "Dialog", "class": "Unity", "initialClass": "Unity", "at": [100, 100], "size": [300, 200], "workspace": {"id": 1, "name": "1"}},
 {"address": "0x5581c", "mapped": false, "title": "Ghost", "class": "Unity", "at": [0, 0], "size": [3000, 3000], "workspace": {"id": 1, "name": "1"}},
 {"address": "0x5581d", "mapped": true, "title": "", "class": "empty-title", "at": [0, 0], "size": [10, 10], "workspace": {"id": 1, "name": "1"}},
 {"address": "0x5581e", "mapped": true, "title": "Terminal éàü with a very long title that goes beyond the eighty characters limit of the launcher entries", "class": "kitty", "initialClass": "kitty", "at": [5, 5], "size": [600, 400], "workspace": {"id": 4, "name": "4"}},
 {"address": "0x5581f", "mapped": true, "title": "Elsewhere", "class": "com.example.Foo", "at": [1, 2], "size": [3, 4], "workspace": {"id": 5, "name": "5"}},
 {"address": "0x55820", "mapped": true, "title": "Zen Browser", "class": "zen", "initialClass": "zen", "at": [0, 0], "size": [800, 600], "workspace": {"id": -98, "name": "special:magic"}}
]"#;

const PRESETS: &str = r#"{
 "defaults": {"dir": "~/Videos", "fps": 60, "quality": "very_high", "codec": "auto", "container": "mp4", "cursor": true},
 "presets": [
  {"id": "unity", "name": "Unity", "subtitle": "Unity editor · FMOD · 1080p60 · no cursor", "icon": "unityhub", "keywords": ["unity", "game"],
   "window": {"class": "^Unity$"}, "audioSource": "app", "audioAppName": "FMOD Ex App", "resolution": "1920x1080", "cursor": false, "dir": "~/Videos/Unity"},
  {"id": "screen", "monitor": "DP-1", "audioSource": "desktop+mic", "container": "mkv", "fps": 30},
  {"id": "zone"},
  {"id": "off", "enabled": false, "name": "Disabled"},
  {"id": "gone", "name": "Gone", "window": {"class": "^Nope$"}},
  {"id": "nomon", "name": "NoMon", "monitor": "DP-9"},
  {"id": "by-title", "name": "By title", "window": {"title": "terminal"}, "audioSource": "app+mic"},
  {"id": "raw", "name": "Raw", "monitor": "HDMI-A-1", "audioSource": "app:Unity|default_input", "codec": "h264", "quality": "high"},
  {"id": "it's", "name": "Quote'd", "subtitle": "", "monitor": "VIRT-1", "audioSource": "none"}
 ]
}"#;

fn setup(b: &Bench) {
    b.presets(PRESETS);
    b.fake("clients.json", CLIENTS);
    b.fake("monitors.json", MONITORS);
    b.fake("app_audio", "Zen\nSpotify\n\n  Unity  \n");
    let apps = b.home.join(".local/share/applications");
    std::fs::create_dir_all(&apps).unwrap();
    std::fs::write(
        apps.join("unity.desktop"),
        "[Desktop Entry]\nIcon=unityhub\nStartupWMClass=Unity\n[Desktop Action x]\nIcon=other\n",
    )
    .unwrap();
    std::fs::write(apps.join("foo.desktop"), "[Desktop Entry]\nIcon=fooicon\n").unwrap();
    std::fs::write(
        apps.join("kitty.desktop"),
        "[Desktop Entry]\nIcon=kitty-icon\r\n",
    )
    .unwrap();
}

macro_rules! need_python {
    () => {
        if !python_available() {
            eprintln!("python3 missing: test skipped");
            return;
        }
    };
}

fn both(b: &Bench, args: &[&str]) -> (String, String) {
    let py = b.python(args).output().unwrap();
    let rs = b.rust(args).output().unwrap();
    assert_eq!(
        py.status.code(),
        rs.status.code(),
        "code de sortie de {args:?}"
    );
    (text(&py), text(&rs))
}

#[test]
fn items_at_rest_identical() {
    need_python!();
    let b = Bench::new();
    setup(&b);
    let (py, rs) = both(&b, &["items"]);
    assert!(
        py.contains("preset:unity") && py.contains("monitor:VIRT-1") && py.contains("\"badge\"")
    );
    assert_eq!(py, rs);
}

#[test]
fn items_without_config_identical() {
    need_python!();
    let b = Bench::new();
    // Neither presets nor hyprctl: only "Region…".
    let (py, rs) = both(&b, &["items"]);
    assert_eq!(py, rs);
    let b = Bench::new();
    b.presets("not json");
    b.fake("clients.json", "not json either");
    let (py, rs) = both(&b, &["items"]);
    assert_eq!(py, rs);
}

fn seed_state(b: &Bench, pid: u32, paused_at: Option<f64>) {
    std::fs::create_dir_all(b.run.join("poste-record")).unwrap();
    let paused = paused_at.map(|p| p.to_string()).unwrap_or("null".into());
    std::fs::write(
        b.state_file(),
        format!(
            r#"{{"pid": {pid}, "started": 1700000000000.0, "pausedAt": {paused}, "pausedTotal": 2500.0, "file": "/run/media/it's/Records/Unity 2026-01-01 10-00-00.mp4", "label": "Unity é"}}"#
        ),
    )
    .unwrap();
}

/// A living process that serves as a fake recorder (never signalled by these tests).
fn sleeper() -> std::process::Child {
    std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap()
}

#[test]
fn status_and_items_during_a_recording() {
    need_python!();
    let b = Bench::new();
    setup(&b);
    let mut s = sleeper();
    let ms_re = regex::Regex::new(r#"\"recordMs\": \d+"#).unwrap();
    for paused in [Some(1700000010000.0), None] {
        seed_state(&b, s.id(), paused);
        let (py, rs) = both(&b, &["items"]);
        assert!(py.contains("\"id\": \"stop\""));
        assert_eq!(py, rs);
        let (py, rs) = both(&b, &["status"]);
        if paused.is_some() {
            // While paused the time is frozen: text identical byte for byte.
            assert_eq!(py, rs);
            assert!(py.contains("\"recordMs\": 7500"));
        } else {
            let ms = |t: &str| -> f64 {
                let v: serde_json::Value = serde_json::from_str(t).unwrap();
                assert_eq!(v["recording"], true);
                assert_eq!(v["paused"], false);
                v["recordMs"].as_f64().unwrap()
            };
            assert!((ms(&py) - ms(&rs)).abs() < 3000.0);
            let strip = |t: &str| ms_re.replace(t, "MS").into_owned();
            assert_eq!(strip(&py), strip(&rs));
        }
    }
    // Process gone: at rest.
    let _ = s.kill();
    let _ = s.wait();
    let (py, rs) = both(&b, &["status"]);
    assert_eq!(py, "{\"t\": \"status\", \"recording\": false}\n");
    assert_eq!(py, rs);
    // Unreadable state file: at rest too.
    std::fs::write(b.state_file(), "{").unwrap();
    let (py, rs) = both(&b, &["status"]);
    assert_eq!(py, rs);
}

#[test]
fn help_identical_without_command_or_unknown_command() {
    need_python!();
    let b = Bench::new();
    for args in [&[][..], &["nimporte"][..], &["start"][..]] {
        let py = b.python(args).output().unwrap();
        let rs = b.rust(args).output().unwrap();
        assert_eq!(py.status.code(), Some(2));
        assert_eq!(rs.status.code(), Some(2));
        assert_eq!(text(&py), text(&rs));
    }
}

struct Run {
    code: Option<i32>,
    stdout: String,
    journal: String,
    files: Vec<String>,
    pick: Option<String>,
}

/// Plays `start` with one implementation, waits for the end of the supervisor if `notify` is expected.
fn play(b: &Bench, rust: bool, args: &[&str], mode: &str, notify: bool) -> Run {
    b.clear_journal();
    b.empty_videos();
    let _ = std::fs::remove_dir_all(b.run.join("poste-record"));
    b.fake("gsr_mode", mode);
    let out = if rust { b.rust(args) } else { b.python(args) }
        .output()
        .unwrap();
    if notify {
        b.wait_notification();
    } else {
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    let pick = std::fs::read_to_string(b.run.join("poste-record/pick.json")).ok();
    Run {
        code: out.status.code(),
        stdout: text(&out),
        journal: norm_stamps(&b.journal()),
        files: file_names(&b.videos)
            .into_iter()
            .chain(
                file_names(&b.videos.join("Unity"))
                    .into_iter()
                    .map(|n| format!("Unity/{n}")),
            )
            .chain(
                file_names(&b.videos.join("Sous"))
                    .into_iter()
                    .map(|n| format!("Sous/{n}")),
            )
            .collect(),
        pick: pick.map(|p| {
            regex::Regex::new(r#""at": [0-9.e+]+"#)
                .unwrap()
                .replace(&p, "AT")
                .into_owned()
        }),
    }
}

fn same_start(b: &Bench, args: &[&str], mode: &str, notify: bool) -> Run {
    let py = play(b, false, args, mode, notify);
    let rs = play(b, true, args, mode, notify);
    assert_eq!(py.code, rs.code, "code de {args:?}");
    assert_eq!(py.stdout, rs.stdout, "sortie de {args:?}");
    assert_eq!(py.journal, rs.journal, "journal de {args:?}");
    assert_eq!(py.files, rs.files, "files of {args:?}");
    assert_eq!(py.pick, rs.pick, "pick de {args:?}");
    rs
}

#[test]
fn start_unity_preset_identical() {
    need_python!();
    let b = Bench::new();
    setup(&b);
    let r = same_start(&b, &["start", "--preset", "unity"], "quick", true);
    assert!(r.journal.contains("app:FMOD Ex App"));
    assert!(r.journal.contains("-cursor\tno"));
    assert!(r.journal.contains("-s\t1920x1080"));
    assert!(r.journal.contains("Recording saved"));
}

#[test]
fn start_targets_and_sounds_identical() {
    need_python!();
    let b = Bench::new();
    setup(&b);
    b.fake("slurp", "10,-20 640x480\n");
    let cases: Vec<Vec<&str>> = vec![
        vec!["start", "--preset", "screen"],
        vec!["start", "--preset", "zone"],
        vec!["start", "--preset", "by-title"],
        vec!["start", "--preset", "raw"],
        vec!["start", "--preset", "it's"],
        vec!["start", "--preset", "unity", "--audio", "none"],
        vec!["start", "--preset", "unity", "--audio", "desktop+mic"],
        vec!["start", "--window", "0x55820"],
        vec!["start", "--window", "0x55820", "--audio", "app+mic"],
        vec!["start", "--window", "0x5581e", "--audio", "app"],
        vec!["start", "--window", "0x5581a", "--audio", "mic"],
        vec!["start", "--monitor", "DP-1"],
        vec!["start", "--monitor", "HDMI-A-1", "--audio", "desktop"],
        vec![
            "start",
            "--monitor",
            "weird/name:1",
            "--audio",
            "app:Zen|default_input",
        ],
        vec!["start", "--region"],
        vec!["start", "--region", "--audio", "none"],
        vec!["start", "--audio", "none", "--monitor", "DP-1"],
    ];
    for args in cases {
        same_start(&b, &args, "quick", true);
    }
}

#[test]
fn start_edge_cases_identical() {
    need_python!();
    let b = Bench::new();
    setup(&b);
    // Unknown preset, window not found, window gone: notification and code 1.
    for args in [
        vec!["start", "--preset", "unknown"],
        vec!["start", "--preset", "gone"],
        vec!["start", "--preset", "off"],
        vec!["start", "--window", "0xdead"],
    ] {
        let r = same_start(&b, &args, "quick", false);
        assert_eq!(r.code, Some(1));
        assert!(r.journal.contains("omarchy-notification-send"));
        assert!(!r.journal.contains("gpu-screen-recorder\t-w"));
    }
    // Region cancelled: nothing happens, code 0.
    let r = same_start(&b, &["start", "--region"], "quick", false);
    assert_eq!(r.code, Some(0));
    assert!(!r.journal.contains("gpu-screen-recorder\t-w"));
    b.fake("slurp", "anything");
    same_start(&b, &["start", "--region"], "quick", false);
}

#[test]
fn start_failures_identical() {
    need_python!();
    let b = Bench::new();
    setup(&b);
    for mode in ["fail", "nofile"] {
        let r = same_start(&b, &["start", "--monitor", "DP-1"], mode, true);
        assert!(r.journal.contains("Recording failed"), "{}", r.journal);
    }
}

#[test]
fn start_config_particular_defaults() {
    need_python!();
    let b = Bench::new();
    b.fake("monitors.json", MONITORS);
    // Folder on an absent disk: fallback to ~/Videos; mkv container: no AAC; floating fps.
    b.presets(
        r#"{"defaults": {"dir": "/mnt/absent-xyz/Records", "container": "mkv", "fps": 59.94, "cursor": null, "audio": "desktop+mic", "codec": "h264"}, "presets": []}"#,
    );
    let r = same_start(&b, &["start", "--monitor", "DP-1"], "quick", true);
    assert!(r.journal.contains("-f\t59.94") && r.journal.contains("-cursor\tno"));
    assert!(!r.journal.contains("aac"));
    // Folder whose parent exists: created.
    b.presets(r#"{"defaults": {"dir": "~/Videos/Sous"}}"#);
    let r = same_start(&b, &["start", "--monitor", "DP-1"], "quick", true);
    assert!(r.journal.contains("Sous/Screen DP-1 STAMP.mp4"));
    // Very long target name or full of forbidden characters.
    b.presets("{}");
    let long = "é".repeat(100) + "/\\:*?";
    same_start(&b, &["start", "--monitor", &long], "quick", true);
}

#[test]
fn opening_of_the_last_recording_identical() {
    need_python!();
    let b = Bench::new();
    setup(&b);
    let touch = |dir: &std::path::Path, name: &str, secs: u64| {
        std::fs::create_dir_all(dir).unwrap();
        let f = std::fs::File::create(dir.join(name)).unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
            .unwrap();
    };
    // No recording: notification and code 1.
    let (py, rs) = both(&b, &["open-last"]);
    assert_eq!(py, rs);
    let j = b.journal();
    assert_eq!(j.matches("No recording yet").count(), 2, "{j}");
    // With videos: the most recent, including by preset.
    touch(&b.videos, "old.mp4", 1_000_000_000);
    touch(&b.videos, "recent.MKV", 1_700_000_000);
    touch(&b.videos, "notes.txt", 1_800_000_000);
    touch(&b.videos.join("Unity"), "u.webm", 1_600_000_000);
    for args in [
        vec!["open-last"],
        vec!["open-last", "--preset", "unity"],
        vec!["open-last", "--preset", "unknown"],
        vec!["open-dir"],
        vec!["open-dir", "--preset", "unity"],
    ] {
        b.clear_journal();
        let py = b.python(&args).output().unwrap();
        b.wait_for("xdg-open", || b.journal().contains("xdg-open"));
        let jp = b.journal();
        b.clear_journal();
        let rs = b.rust(&args).output().unwrap();
        b.wait_for("xdg-open", || b.journal().contains("xdg-open"));
        assert_eq!(py.status.code(), rs.status.code());
        assert_eq!(jp, b.journal(), "{args:?}");
    }
    // The file being recorded is excluded.
    let mut s = sleeper();
    let busy = format!("{}/busy.mp4", b.videos.display());
    touch(&b.videos, "busy.mp4", 1_900_000_000);
    std::fs::create_dir_all(b.run.join("poste-record")).unwrap();
    std::fs::write(
        b.state_file(),
        format!(r#"{{"pid": {}, "started": 1.0, "pausedAt": null, "pausedTotal": 0, "file": "{busy}", "label": "x"}}"#, s.id()),
    )
    .unwrap();
    b.clear_journal();
    b.python(&["open-last"]).output().unwrap();
    b.wait_for("xdg-open", || b.journal().contains("xdg-open"));
    let jp = b.journal();
    assert!(!jp.contains("busy.mp4") && jp.contains("recent.MKV"));
    b.clear_journal();
    b.rust(&["open-last"]).output().unwrap();
    b.wait_for("xdg-open", || b.journal().contains("xdg-open"));
    assert_eq!(jp, b.journal());
    let _ = s.kill();
    let _ = s.wait();
}

#[test]
fn hotkey_without_recording_opens_the_launcher() {
    need_python!();
    let b = Bench::new();
    b.clear_journal();
    let py = b.python(&["hotkey"]).output().unwrap();
    let jp = b.journal();
    b.clear_journal();
    let rs = b.rust(&["hotkey"]).output().unwrap();
    assert_eq!(py.status.code(), rs.status.code());
    assert_eq!(jp, b.journal());
    assert!(jp.contains("omarchy-shell\tposte.island\tlauncherIn\trecord"));
}

#[test]
fn share_picker_identical() {
    need_python!();
    let b = Bench::new();
    let list = "12[HC>]kitty[HT>]shell[HE>]55aa[HA>]34[HC>]Unity[HT>]Project[HE>]0x5581[HA>]";
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let cases: Vec<(Option<String>, f64)> = vec![
        (Some(r#""0x5581""#.into()), now),
        (Some(r#""0X5581""#.into()), now),
        (Some(r#""22913""#.into()), now),
        (Some(r#""0x9999""#.into()), now),
        (Some(r#""0x5581""#.into()), now - 30.0),
        (Some("null".into()), now),
        (Some(r#""not a number""#.into()), now),
        (None, now),
    ];
    for (address, at) in cases {
        let mut outs = Vec::new();
        for rust in [false, true] {
            let _ = std::fs::remove_dir_all(b.run.join("poste-record"));
            b.clear_journal();
            if let Some(a) = &address {
                std::fs::create_dir_all(b.run.join("poste-record")).unwrap();
                std::fs::write(
                    b.run.join("poste-record/pick.json"),
                    format!(r#"{{"address": {a}, "at": {at}}}"#),
                )
                .unwrap();
            }
            let mut c = if rust {
                // Through the stable path put in xdph.conf: the module's shell shortcut.
                let mut c = b.rust(&["share-picker", "--arg"]);
                c.env("XDPH_WINDOW_SHARING_LIST", list);
                c
            } else {
                let mut c = b.base_python_script("poste-share-picker.py", &["--arg"]);
                c.env("XDPH_WINDOW_SHARING_LIST", list);
                c
            };
            let o = c.output().unwrap();
            outs.push((
                o.status.code(),
                text(&o),
                b.journal(),
                b.run.join("poste-record/pick.json").exists(),
                std::fs::read_to_string(b.run.join("poste-record/picker.log")).ok(),
            ));
        }
        assert_eq!(outs[0], outs[1], "address {address:?}, at {at}");
    }
}
