//! Common test bench: temporary HOME, XDG_RUNTIME_DIR and PATH, fake outside programs.
//! Nothing touches the real session: neither gpu-screen-recorder, nor hyprctl, nor notifications.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

pub const FAKES: [&str; 8] = [
    "hyprctl",
    "gpu-screen-recorder",
    "slurp",
    "omarchy-notification-send",
    "xdg-open",
    "omarchy-shell",
    "hyprland-preview-share-picker",
    "notify-send",
];

pub struct Bench {
    pub tmp: tempfile::TempDir,
    pub home: PathBuf,
    pub run: PathBuf,
    pub videos: PathBuf,
    pub fakes: PathBuf,
    pub work: PathBuf,
}

pub fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn python_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

impl Bench {
    pub fn new() -> Bench {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let b = Bench {
            home: root.join("home"),
            run: root.join("run"),
            videos: root.join("home/Videos"),
            fakes: root.join("fakes"),
            work: root.join("work"),
            tmp,
        };
        for d in [&b.home, &b.run, &b.videos, &b.fakes, &b.work] {
            std::fs::create_dir_all(d).unwrap();
        }
        let src = manifest_dir().join("tests/fakes/fake.sh");
        for name in FAKES {
            std::fs::copy(&src, b.fakes.join(name)).unwrap();
        }
        b
    }

    /// Common environment, inheriting nothing from the real session.
    fn base(&self, mut c: Command) -> Command {
        c.env_clear()
            .env("HOME", &self.home)
            .env("XDG_RUNTIME_DIR", &self.run)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_VIDEOS_DIR", &self.videos)
            .env("FAKE_DIR", &self.work)
            .env("TZ", "UTC")
            .env("LANG", "C.UTF-8")
            .env("PYTHONIOENCODING", "utf-8")
            .env("PATH", format!("{}:/usr/bin:/bin", self.fakes.display()));
        c
    }

    /// The Rust binary, with the fake programs designated by the test variables.
    pub fn rust(&self, args: &[&str]) -> Command {
        let mut c = self.base(Command::new(env!("CARGO_BIN_EXE_poste-record")));
        c.args(args)
            .env("POSTE_RECORD_GSR", self.fakes.join("gpu-screen-recorder"))
            .env("POSTE_RECORD_HYPRCTL", self.fakes.join("hyprctl"))
            .env(
                "POSTE_RECORD_NOTIFY",
                self.fakes.join("omarchy-notification-send"),
            );
        c
    }

    /// The original Python script, with the same fake programs (through the PATH).
    pub fn python(&self, args: &[&str]) -> Command {
        let mut c = self.base(Command::new("python3"));
        c.arg(manifest_dir().join("poste-record.py")).args(args);
        c
    }

    /// A Python script of the module folder, with the same environment.
    pub fn base_python_script(&self, script: &str, args: &[&str]) -> Command {
        let mut c = self.base(Command::new("python3"));
        c.arg(manifest_dir().join(script)).args(args);
        c
    }

    pub fn fake(&self, name: &str, content: &str) {
        std::fs::write(self.work.join(name), content).unwrap();
    }

    pub fn journal(&self) -> String {
        std::fs::read_to_string(self.work.join("journal")).unwrap_or_default()
    }

    pub fn clear_journal(&self) {
        let _ = std::fs::remove_file(self.work.join("journal"));
    }

    pub fn presets(&self, json: &str) {
        let p = self.home.join(".config/poste/record/presets.json");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, json).unwrap();
    }

    pub fn state_file(&self) -> PathBuf {
        self.run.join("poste-record/state.json")
    }

    /// Waits for a condition to be true (5 s at most, polling every 20 ms).
    pub fn wait_for(&self, what: &str, f: impl Fn() -> bool) {
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            if f() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "timeout while waiting for: {what}\njournal:\n{}",
            self.journal()
        );
    }

    /// Waits for the fake gsr (hold mode) to have logged its arguments and taken over its
    /// signals: the state is written as soon as it is launched, before.
    pub fn wait_gsr_ready(&self) {
        self.wait_for("the fake gsr ready", || {
            self.journal().contains("gsr-ready\n")
        });
    }

    /// Waits for the supervisor's final notification.
    pub fn wait_notification(&self) {
        self.wait_for("the notification", || {
            self.journal().contains("omarchy-notification-send\t")
        });
        // The supervisor cleans up before notifying: the state has disappeared.
        self.wait_for("the end of the state", || !self.state_file().exists());
    }

    pub fn empty_videos(&self) {
        let _ = std::fs::remove_dir_all(&self.videos);
        std::fs::create_dir_all(&self.videos).unwrap();
    }
}

pub fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Replaces the dates of file names (the second can change between two attempts).
pub fn norm_stamps(s: &str) -> String {
    regex::Regex::new(r"\d{4}-\d\d-\d\d \d\d-\d\d-\d\d")
        .unwrap()
        .replace_all(s, "STAMP")
        .into_owned()
}

pub fn file_names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v.iter().map(|n| norm_stamps(n)).collect()
}
