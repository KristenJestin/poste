//! Reload of the island (`omarchy restart shell`) and of Hyprland (`hyprctl reload`)
//! after `poste sys sync`, depending on the files changed by the pull; the shell alone after
//! `poste sys update` moved a plugin.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

/// What must be reloaded.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Need {
    pub shell: bool,
    pub hypr: bool,
}

impl Need {
    pub fn any(&self) -> bool {
        self.shell || self.hypr
    }

    /// Adds what a changed path asks for, relative to the root of the repository (poste repository
    /// or data repository, same layout).
    pub fn add_path(&mut self, path: &str) {
        let c: Vec<&str> = path.split('/').collect();
        match c.as_slice() {
            ["config", ".config", "omarchy" | "poste-island" | "fontconfig", ..] => {
                self.shell = true
            }
            ["config", ".config", "hypr", ..] => self.hypr = true,
            ["modules" | "addons", _, "island", ..] => self.shell = true,
            // module.toml: its `[[line]]` entries may target Hyprland's config.
            ["modules" | "addons", _, "hypr", ..] | ["modules" | "addons", _, "module.toml"] => {
                self.hypr = true
            }
            _ => {}
        }
    }
}

/// Current commit of a repository.
pub fn head(dir: &Path) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("cannot run git")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Adds to `need` the files changed between two commits.
pub fn add_changes(need: &mut Need, dir: &Path, from: &str, to: &str) -> Result<()> {
    if from.is_empty() || from == to {
        return Ok(());
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["diff", "--name-only", from, to])
        .output()
        .context("cannot run git")?;
    for l in String::from_utf8_lossy(&out.stdout).lines() {
        need.add_path(l);
    }
    Ok(())
}

/// A screen recording is running (restarting the island would cut it): the recorder's process,
/// or the state of `poste record` when the module is there.
fn recording() -> bool {
    let process = Command::new("pgrep")
        .args(["-f", "gpu-screen-recorder"])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    process
        || Command::new("poste")
            .args(["record", "status"])
            .stderr(Stdio::null())
            .output()
            .ok()
            .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
            .is_some_and(|v| v["recording"] == true)
}

/// Restarts the shell (`omarchy restart shell`), never during a screen recording. False if it
/// was not restarted (recording, or the command failed), with the reason printed.
pub fn restart_shell() -> bool {
    if recording() {
        println!(
            "Screen recording in progress: shell not restarted, run `omarchy restart shell` once it is over."
        );
        return false;
    }
    println!("\n$ omarchy restart shell");
    match Command::new("omarchy").args(["restart", "shell"]).status() {
        Ok(s) if s.success() => true,
        _ => {
            eprintln!("poste sys: `omarchy restart shell` failed");
            false
        }
    }
}

fn launch(args: &[&str]) {
    match Command::new(args[0]).args(&args[1..]).status() {
        Ok(s) if s.success() => {}
        _ => eprintln!("poste sys: `{}` failed", args.join(" ")),
    }
}

/// Reloads what `need` asks for: without `yes`, after confirmation. The island is never
/// restarted during a screen recording.
pub fn apply(
    need: &Need,
    yes: bool,
    dry_run: bool,
    confirm: &dyn Fn(&str) -> Result<bool>,
) -> Result<()> {
    if !need.any() {
        return Ok(());
    }
    let mut cmds = Vec::new();
    if need.shell {
        cmds.push("omarchy restart shell");
    }
    if need.hypr {
        cmds.push("hyprctl reload");
    }
    let list = cmds.join("` and `");
    if dry_run {
        println!("\nDry run: reload not run (`{list}`).");
        return Ok(());
    }
    if !yes && !confirm(&format!("\nReload now (`{list}`)? [y/N] "))? {
        println!("To reload later: `{list}`.");
        return Ok(());
    }
    if need.hypr {
        launch(&["hyprctl", "reload"]);
    }
    if need.shell {
        restart_shell();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn need(paths: &[&str]) -> Need {
        let mut n = Need::default();
        for p in paths {
            n.add_path(p);
        }
        n
    }

    #[test]
    fn island_files_restart_the_shell() {
        let n = need(&[
            "config/.config/omarchy/plugins/poste.island/Island.qml",
            "addons/vocaline/island/VocalineState.qml",
        ]);
        assert_eq!(
            n,
            Need {
                shell: true,
                hypr: false
            }
        );
    }

    #[test]
    fn hypr_files_and_module_lines_reload_hyprland() {
        assert_eq!(
            need(&["config/.config/hypr/looknfeel.lua"]),
            Need {
                shell: false,
                hypr: true
            }
        );
        assert_eq!(
            need(&["modules/record/hypr/record.lua"]),
            Need {
                shell: false,
                hypr: true
            }
        );
        assert_eq!(
            need(&["modules/record/module.toml"]),
            Need {
                shell: false,
                hypr: true
            }
        );
    }

    #[test]
    fn addon_files_and_data_choices() {
        assert_eq!(
            need(&["addons/vocaline/hypr/vocaline.lua"]),
            Need {
                shell: false,
                hypr: true
            }
        );
        assert!(!need(&["addons/melodie/README.md"]).any());
    }

    #[test]
    fn other_files_need_nothing() {
        let n = need(&[
            "README.md",
            "modules/usage/src/main.rs",
            "config/.config/git/config",
            "modules/files/README.md",
        ]);
        assert!(!n.any());
    }
}
