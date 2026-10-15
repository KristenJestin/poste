//! The real binary, end to end, on a temporary home folder: index built by the first search,
//! answer in the launcher's format, exclusion files, refresh, recent files. Nothing touches the
//! real `$HOME`, the D-Bus bus or the clipboard; `xdg-open` is a fake that only writes down
//! what it was given, and no process is left after the test.

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

struct Sandbox {
    _dir: tempfile::TempDir,
    home: PathBuf,
    bin: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home");
        for f in [
            "notes/pancake-recipe.md",
            "Projects/game/Assets/Player.cs",
            "Projects/game/Library/ShaderCache/player-cache.bin",
            "Projects/site/node_modules/player-lib/index.js",
            "Projects/site/build/player-build.js",
        ] {
            let p = home.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, "x").unwrap();
        }
        // Ignored by git, found all the same.
        fs::write(home.join("Projects/game/.gitignore"), "Assets/\n").unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let fake = bin.join("xdg-open");
        fs::write(
            &fake,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$1\" >> {}\n",
                dir.path().join("opened").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            _dir: dir,
            home,
            bin,
        }
    }

    fn cmd(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_poste-files"))
            .args(args)
            .env("HOME", &self.home)
            .env("PATH", &self.bin)
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_CACHE_HOME")
            .env_remove("XDG_STATE_HOME")
            .env_remove("XDG_DATA_HOME")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("WAYLAND_DISPLAY")
            .output()
            .unwrap()
    }

    fn json(&self, args: &[&str]) -> Vec<Value> {
        let out = self.cmd(args);
        assert!(out.status.success());
        serde_json::from_slice::<Value>(&out.stdout)
            .unwrap()
            .as_array()
            .unwrap()
            .clone()
    }

    fn search(&self, q: &str) -> Vec<Value> {
        self.json(&["search", "--json", q])
    }

    fn index(&self) -> PathBuf {
        self.home.join(".cache/poste/files.idx")
    }

    /// Moves the date of the index back: the next `refresh` without `--force` rebuilds it, and
    /// a search starts a walk.
    fn age_index(&self) {
        let f = fs::File::options().write(true).open(self.index()).unwrap();
        f.set_modified(std::time::SystemTime::now() - Duration::from_secs(120))
            .unwrap();
    }

    /// Waits for the background walks asked for to end (marker gone, lock free).
    fn wait_idle(&self) {
        let cache = self.home.join(".cache/poste");
        wait_for("background walk ended", || {
            !cache.join("files.pending").exists()
                && fs::File::options()
                    .write(true)
                    .open(cache.join("files.lock"))
                    .map_or(true, |f| f.try_lock().is_ok())
        });
    }

    fn opened(&self) -> String {
        fs::read_to_string(self.home.parent().unwrap().join("opened")).unwrap_or_default()
    }
}

impl Drop for Sandbox {
    /// No walk left running in a folder about to be deleted.
    fn drop(&mut self) {
        if !std::thread::panicking() {
            self.wait_idle();
        }
    }
}

fn ids(v: &[Value]) -> Vec<String> {
    v.iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect()
}

fn id(p: &Path) -> String {
    format!("file:{}", p.display())
}

/// Waits (5 s at most) for `ok`.
fn wait_for(what: &str, ok: impl Fn() -> bool) {
    let start = Instant::now();
    while !ok() {
        assert!(start.elapsed() < Duration::from_secs(5), "{what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn end_to_end() {
    let s = Sandbox::new();
    // Nothing typed and nothing recent: an empty list, and no walk.
    assert!(s.json(&["home", "--json"]).is_empty());
    assert!(!s.index().exists(), "home walks nothing");

    // The first search builds the index.
    let pancake = s.home.join("notes/pancake-recipe.md");
    let v = s.search("pancake");
    assert_eq!(v[0]["id"], id(&pancake));
    assert_eq!(v[0]["subtitle"], "~/notes");
    assert_eq!(v[0]["actions"].as_array().unwrap().len(), 4);
    assert!(s.index().exists());

    // Default exclusions; the .gitignore is not read.
    let player = s.home.join("Projects/game/Assets/Player.cs");
    assert_eq!(ids(&s.search("player")), [id(&player)]);

    // A file created afterwards: missing until the next refresh.
    let new = s.home.join("notes/shopping-list.txt");
    fs::write(&new, "x").unwrap();
    assert!(s.search("shopping").is_empty());
    // Recent index: refresh does nothing, --force does.
    let out = s.cmd(&["refresh"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("nothing to do"));
    assert!(s.search("shopping").is_empty());
    assert!(s.cmd(&["refresh", "--force"]).status.success());
    assert_eq!(ids(&s.search("shopping")), [id(&new)]);

    // Opening goes through `poste files open`: the file is opened and becomes recent.
    assert!(s.cmd(&["open", new.to_str().unwrap()]).status.success());
    wait_for("xdg-open called", || {
        s.opened().contains("shopping-list.txt")
    });
    assert_eq!(ids(&s.json(&["home", "--json"])), [id(&new)]);

    // Exclusions of the machine, in its data repository (lifts build/) then local (excludes
    // notes/).
    let data = s.home.join("data-repo");
    fs::create_dir_all(data.join("files")).unwrap();
    fs::write(data.join("files/ignore"), "!build/\n").unwrap();
    fs::create_dir_all(s.home.join(".config/poste")).unwrap();
    fs::write(
        s.home.join(".config/poste/config.toml"),
        format!("data = \"{}\"\n", data.display()),
    )
    .unwrap();
    let local = s.home.join(".local/share/poste/files/ignore");
    fs::create_dir_all(local.parent().unwrap()).unwrap();
    fs::write(&local, "/notes/\n").unwrap();
    s.age_index();
    // In the background: the command returns, the walk replaces the index afterwards.
    assert!(s.cmd(&["refresh", "--background"]).status.success());
    let build = s.home.join("Projects/site/build/player-build.js");
    wait_for("build/ indexed", || {
        ids(&s.search("player")).contains(&id(&build))
    });
    assert!(s.search("pancake").is_empty());

    // A query starting with "-" is a query, not an option.
    let v = s.json(&["search", "--json", "-rf"]);
    assert!(v.iter().all(|e| e["id"] != "files-error"), "{v:#?}");

    // An index in another format is rebuilt by the next search.
    s.wait_idle();
    fs::write(s.index(), "poste-files-index 0\n").unwrap();
    let mut found = ids(&s.search("player"));
    found.sort();
    assert_eq!(found, [id(&player), id(&build)]);

    // Invalid pattern: one error entry for the launcher, exit code 0.
    s.wait_idle();
    fs::write(data.join("files/ignore"), "[z-a]\n").unwrap();
    fs::remove_file(s.index()).unwrap();
    let v = s.search("player");
    assert_eq!(v[0]["id"], "files-error");
    assert!(v[0]["subtitle"]
        .as_str()
        .unwrap()
        .contains("data-repo/files/ignore, line 1"));

    let out = s.cmd(&["status"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("Roots: ~"), "{text}");
}

#[test]
fn search_on_an_old_index_restarts_the_walk() {
    let s = Sandbox::new();
    assert!(!s.search("pancake").is_empty());
    let new = s.home.join("notes/shopping-list.txt");
    fs::write(&new, "x").unwrap();
    s.age_index();
    // Answers with the index in place, then the restarted walk adds the new file.
    assert!(s.search("shopping").is_empty());
    wait_for("new file indexed", || {
        ids(&s.search("shopping")) == [id(&new)]
    });
}
