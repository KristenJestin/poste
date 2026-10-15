//! Hand-over to the `poste-sys` of the new root.
//!
//! `sync` and `use` point `current` at another root (published version, pulled clone)
//! then run `setup`. This `setup` must be the one of the new root: something new in
//! `poste sys` (a kind of systemd unit, a placing rule) arrives with the version that
//! uses it, and the old program would miss it. Once `current` is set, if the `poste-sys` of the
//! new root differs from the running one and knows the hidden command `finish`, it is
//! handed over to (`exec`): it runs `setup` from `current`, prunes the old versions if
//! asked, then reloads the island and Hyprland. `finish` never hands over again.
//!
//! Otherwise (same program, version from before the hand-over, launch impossible), the rest is done
//! here, as before.
//!
//! `finish` is a contract between versions: its options do not change, they are only added to.

use crate::reload::Need;
use crate::setup;
use crate::util::sha256_file;
use crate::versions::{self, Switched};
use anyhow::Result;
use poste_core::Env;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The `poste-sys` of a root (built by `cargo` in the clone, shipped in a version).
pub fn binary(root: &Path) -> PathBuf {
    root.join("target/release/poste-sys")
}

/// The running program. No hand-over in tests: the test binary is not
/// `poste-sys`.
fn running() -> Option<PathBuf> {
    if cfg!(test) {
        None
    } else {
        std::env::current_exe().ok()
    }
}

/// The `poste-sys` of `root`, if it differs from `running`.
fn target(root: &Path, running: &Path) -> Option<PathBuf> {
    let bin = binary(root);
    if !bin.is_file() {
        return None;
    }
    if std::fs::canonicalize(&bin).ok() == std::fs::canonicalize(running).ok() {
        return None;
    }
    // The running program may have been replaced in the meantime (unreadable): different.
    let same = sha256_file(running).is_ok_and(|h| sha256_file(&bin).is_ok_and(|b| b == h));
    (!same).then_some(bin)
}

/// Set in the environment of a `setup` handed over by [`setup_from_clone`]: it never hands over
/// again, and knows the clone is already built.
const SETUP_HANDED_OVER: &str = "POSTE_SYS_SETUP_HANDED_OVER";

/// `setup` from a clone (dev mode): the clone is built first, then, if its `poste-sys` differs
/// from the running one (`poste` in `~/.local/bin` runs the previous build), it is handed over to
/// with the same arguments, so that what is new in `setup` applies in this run, not the next one.
/// Returns whether the clone is built (by this process or the one that handed over).
pub fn setup_from_clone(root: &Path) -> Result<bool> {
    if !root.join("Cargo.toml").is_file() {
        return Ok(false);
    }
    if std::env::var_os(SETUP_HANDED_OVER).is_some() {
        return Ok(true);
    }
    setup::build(root)?;
    let Some(bin) = running().and_then(|r| target(root, &r)) else {
        return Ok(true);
    };
    println!("Handing over to the new build: {}", bin.display());
    let err = Command::new(&bin)
        .args(std::env::args_os().skip(1))
        .env(SETUP_HANDED_OVER, "1")
        .exec();
    eprintln!(
        "poste sys: could not hand over to {} ({err}), going on with this program",
        bin.display()
    );
    Ok(true)
}

/// Does the program know `finish`? (a version from before the hand-over does not know it: rollback
/// to it).
fn knows_finish(bin: &Path) -> bool {
    Command::new(bin)
        .args(["finish", "--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The rest of `sync` and `use` once `current` is set (`old`: the previous root): hands
/// over to the `poste-sys` of `current` if needed, otherwise `versions::finish` here.
pub fn complete(
    env: &Env,
    old: Option<&Path>,
    need: Need,
    yes: bool,
    prune: bool,
    skip_build: bool,
) -> Result<Switched> {
    let root = env.current_link();
    // The clone is built first: its `poste-sys` is the one of the pulled code.
    if !skip_build && root.join("Cargo.toml").is_file() {
        setup::build(&root)?;
    }
    if let Some(bin) = running().and_then(|r| target(&root, &r)) {
        if knows_finish(&bin) {
            println!("\nNew poste-sys ({}): it does the rest.", env.display(&bin));
            let err = finish_command(&bin, old, &need, yes, prune).exec();
            eprintln!(
                "poste sys: cannot run {} ({err}): the rest is done by this program.",
                env.display(&bin)
            );
        }
    }
    let opts = setup::Opts {
        yes,
        skip_build: true,
        ..Default::default()
    };
    versions::finish(env, &root, need, &opts, prune)
}

fn finish_command(bin: &Path, old: Option<&Path>, need: &Need, yes: bool, prune: bool) -> Command {
    let mut c = Command::new(bin);
    c.args(["finish", "--skip-build"]);
    // The previous root: the new program compares the roots again itself (it may know
    // more things to reload than the one that hands over).
    if let Some(o) = old {
        c.arg("--from").arg(o);
    }
    for (on, flag) in [
        (yes, "--yes"),
        (prune, "--prune"),
        (need.shell, "--reload-shell"),
        (need.hypr, "--reload-hypr"),
    ] {
        if on {
            c.arg(flag);
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn setup_from_a_version_builds_nothing() {
        let d = tempfile::tempdir().unwrap();
        assert!(!setup_from_clone(d.path()).unwrap());
    }

    #[test]
    fn hand_over_only_to_a_different_binary() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("root");
        let running = d.path().join("poste-sys");
        fs::write(&running, "old").unwrap();
        // No poste-sys in the root: we stay.
        assert_eq!(target(&root, &running), None);
        fs::create_dir_all(binary(&root).parent().unwrap()).unwrap();
        fs::write(binary(&root), "old").unwrap();
        // The same one (content, or the same file): we stay; another one: we hand over.
        assert_eq!(target(&root, &running), None);
        assert_eq!(target(&root, &binary(&root)), None);
        fs::write(binary(&root), "new").unwrap();
        assert_eq!(target(&root, &running), Some(binary(&root)));
    }

    #[test]
    fn finish_unknown_to_an_old_version() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("poste-sys");
        // A version from before the hand-over: clap refuses the subcommand (code 2).
        fs::write(&bin, "#!/bin/sh\nexit 2\n").unwrap();
        fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        assert!(!knows_finish(&bin));
        fs::write(&bin, "#!/bin/sh\n[ \"$1\" = finish ]\n").unwrap();
        assert!(knows_finish(&bin));
        assert!(!knows_finish(&d.path().join("absent")));
    }

    #[test]
    fn finish_command_passes_the_options() {
        let need = Need {
            shell: true,
            hypr: false,
        };
        let c = finish_command(
            Path::new("/x/poste-sys"),
            Some(Path::new("/v/old")),
            &need,
            true,
            true,
        );
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "finish",
                "--skip-build",
                "--from",
                "/v/old",
                "--yes",
                "--prune",
                "--reload-shell"
            ]
        );
    }
}
