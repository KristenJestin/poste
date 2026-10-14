//! Rechargement de l'île (`omarchy restart shell`) et de Hyprland (`hyprctl reload`)
//! après `poste sys sync`, selon les fichiers changés par le pull.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

/// Ce qu'il faut recharger.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Need {
    pub shell: bool,
    pub hypr: bool,
}

impl Need {
    pub fn any(&self) -> bool {
        self.shell || self.hypr
    }

    /// Ajoute ce que demande un chemin changé, relatif à la racine du dépôt (dépôt poste
    /// ou dépôt de données, même disposition).
    pub fn add_path(&mut self, path: &str) {
        let c: Vec<&str> = path.split('/').collect();
        match c.as_slice() {
            ["config", ".config", "omarchy" | "poste-island" | "fontconfig", ..] => {
                self.shell = true
            }
            ["config", ".config", "hypr", ..] => self.hypr = true,
            ["modules", _, "island", ..] => self.shell = true,
            // module.toml : ses `[[line]]` peuvent viser la config de Hyprland.
            ["modules", _, "hypr", ..] | ["modules", _, "module.toml"] => self.hypr = true,
            _ => {}
        }
    }
}

/// Commit courant d'un dépôt.
pub fn head(dir: &Path) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .context("impossible de lancer git")?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Ajoute à `need` les fichiers changés entre deux commits.
pub fn add_changes(need: &mut Need, dir: &Path, from: &str, to: &str) -> Result<()> {
    if from.is_empty() || from == to {
        return Ok(());
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["diff", "--name-only", from, to])
        .output()
        .context("impossible de lancer git")?;
    for l in String::from_utf8_lossy(&out.stdout).lines() {
        need.add_path(l);
    }
    Ok(())
}

/// Un enregistrement d'écran tourne (redémarrer l'île le couperait).
fn recording() -> bool {
    Command::new("pgrep")
        .args(["-f", "gpu-screen-recorder"])
        .stdout(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn launch(args: &[&str]) {
    match Command::new(args[0]).args(&args[1..]).status() {
        Ok(s) if s.success() => {}
        _ => eprintln!("poste sys : `{}` a échoué", args.join(" ")),
    }
}

/// Recharge ce que `need` demande : sans `yes`, après confirmation. L'île n'est jamais
/// redémarrée pendant un enregistrement d'écran.
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
    let list = cmds.join("` et `");
    if dry_run {
        println!("\nSimulation : rechargement non lancé (`{list}`).");
        return Ok(());
    }
    if !yes && !confirm(&format!("\nRecharger maintenant (`{list}`) ? [o/N] "))? {
        println!("Pour recharger plus tard : `{list}`.");
        return Ok(());
    }
    if need.hypr {
        launch(&["hyprctl", "reload"]);
    }
    if need.shell {
        if recording() {
            println!(
                "Enregistrement d'écran en cours : île non redémarrée, lancez `omarchy restart shell` une fois fini."
            );
        } else {
            launch(&["omarchy", "restart", "shell"]);
        }
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
            "modules/vocaline/island/VocalineState.qml",
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
    fn other_files_need_nothing() {
        let n = need(&[
            "README.md",
            "modules/music/src/main.rs",
            "config/.config/git/config",
            "modules/files/README.md",
        ]);
        assert!(!n.any());
    }
}
