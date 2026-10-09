//! Banc d'essai commun : HOME, XDG_RUNTIME_DIR et PATH temporaires, faux programmes extérieurs.
//! Rien ne touche à la vraie session : ni gpu-screen-recorder, ni hyprctl, ni les notifications.
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

    /// Environnement commun, sans rien hériter de la session réelle.
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

    /// Le binaire Rust, avec les faux programmes désignés par les variables de test.
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

    /// Le script Python d'origine, avec les mêmes faux programmes (par le PATH).
    pub fn python(&self, args: &[&str]) -> Command {
        let mut c = self.base(Command::new("python3"));
        c.arg(manifest_dir().join("poste-record.py")).args(args);
        c
    }

    /// Un script Python du dossier du module, avec le même environnement.
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

    /// Attend qu'une condition soit vraie (5 s au plus, sondage toutes les 20 ms).
    pub fn wait_for(&self, what: &str, f: impl Fn() -> bool) {
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            if f() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "délai dépassé en attendant : {what}\njournal :\n{}",
            self.journal()
        );
    }

    /// Attend la notification finale du superviseur.
    pub fn wait_notification(&self) {
        self.wait_for("la notification", || {
            self.journal().contains("omarchy-notification-send\t")
        });
        // Le superviseur nettoie avant de notifier : l'état a disparu.
        self.wait_for("la fin de l'état", || !self.state_file().exists());
    }

    pub fn empty_videos(&self) {
        let _ = std::fs::remove_dir_all(&self.videos);
        std::fs::create_dir_all(&self.videos).unwrap();
    }
}

pub fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

/// Remplace les dates de nom de fichier (la seconde peut changer entre deux essais).
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
