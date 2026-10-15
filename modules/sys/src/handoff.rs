//! Relais vers le `poste-sys` de la nouvelle racine.
//!
//! `sync` et `use` font pointer `current` sur une autre racine (version publiée, clone tiré)
//! puis lancent `setup`. Ce `setup` doit être celui de la nouvelle racine : une nouveauté de
//! `poste sys` (une sorte d'unité systemd, une règle de pose) arrive avec la version qui s'en
//! sert, et l'ancien programme la raterait. Une fois `current` posé, si le `poste-sys` de la
//! nouvelle racine diffère de celui qui tourne et connaît la commande cachée `finish`, on lui
//! passe la main (`exec`) : il lance `setup` depuis `current`, élague les anciennes versions si
//! demandé, puis recharge l'île et Hyprland. `finish` ne repasse jamais la main.
//!
//! Sinon (même programme, version d'avant le relais, lancement impossible), la suite est faite
//! ici, comme avant.
//!
//! `finish` est un contrat entre versions : ses options ne changent pas, on en ajoute seulement.

use crate::reload::Need;
use crate::setup;
use crate::util::sha256_file;
use crate::versions::{self, Switched};
use anyhow::Result;
use poste_core::Env;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Le `poste-sys` d'une racine (construit par `cargo` dans le clone, livré dans une version).
pub fn binary(root: &Path) -> PathBuf {
    root.join("target/release/poste-sys")
}

/// Le programme qui tourne. Aucun relais dans les tests : le binaire de test n'est pas
/// `poste-sys`.
fn running() -> Option<PathBuf> {
    if cfg!(test) {
        None
    } else {
        std::env::current_exe().ok()
    }
}

/// Le `poste-sys` de `root`, s'il diffère de `running`.
fn target(root: &Path, running: &Path) -> Option<PathBuf> {
    let bin = binary(root);
    if !bin.is_file() {
        return None;
    }
    if std::fs::canonicalize(&bin).ok() == std::fs::canonicalize(running).ok() {
        return None;
    }
    // Le programme qui tourne peut avoir été remplacé entre-temps (illisible) : différent.
    let same = sha256_file(running).is_ok_and(|h| sha256_file(&bin).is_ok_and(|b| b == h));
    (!same).then_some(bin)
}

/// Le programme connaît-il `finish` ? (une version d'avant le relais ne le connaît pas : retour
/// arrière vers elle).
fn knows_finish(bin: &Path) -> bool {
    Command::new(bin)
        .args(["finish", "--help"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// La suite de `sync` et de `use` une fois `current` posé (`old` : la racine d'avant) : passe
/// la main au `poste-sys` de `current` s'il le faut, sinon `versions::finish` ici.
pub fn complete(
    env: &Env,
    old: Option<&Path>,
    need: Need,
    yes: bool,
    prune: bool,
    skip_build: bool,
) -> Result<Switched> {
    let root = env.current_link();
    // Le clone se construit d'abord : son `poste-sys` est celui du code tiré.
    if !skip_build && root.join("Cargo.toml").is_file() {
        setup::build(&root)?;
    }
    if let Some(bin) = running().and_then(|r| target(&root, &r)) {
        if knows_finish(&bin) {
            println!(
                "\nNouveau poste-sys ({}) : c'est lui qui fait la suite.",
                env.display(&bin)
            );
            let err = finish_command(&bin, old, &need, yes, prune).exec();
            eprintln!(
                "poste sys : impossible de lancer {} ({err}) : suite faite par ce programme.",
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
    // L'ancienne racine : le nouveau programme recompare les racines lui-même (il connaît
    // peut-être plus de choses à recharger que celui qui passe la main).
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
    fn relais_seulement_vers_un_binaire_different() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("root");
        let running = d.path().join("poste-sys");
        fs::write(&running, "ancien").unwrap();
        // Pas de poste-sys dans la racine : on reste.
        assert_eq!(target(&root, &running), None);
        fs::create_dir_all(binary(&root).parent().unwrap()).unwrap();
        fs::write(binary(&root), "ancien").unwrap();
        // Le même (contenu, ou le même fichier) : on reste ; un autre : on passe la main.
        assert_eq!(target(&root, &running), None);
        assert_eq!(target(&root, &binary(&root)), None);
        fs::write(binary(&root), "nouveau").unwrap();
        assert_eq!(target(&root, &running), Some(binary(&root)));
    }

    #[test]
    fn finish_inconnu_d_une_ancienne_version() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("poste-sys");
        // Une version d'avant le relais : clap refuse la sous-commande (code 2).
        fs::write(&bin, "#!/bin/sh\nexit 2\n").unwrap();
        fs::set_permissions(&bin, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        assert!(!knows_finish(&bin));
        fs::write(&bin, "#!/bin/sh\n[ \"$1\" = finish ]\n").unwrap();
        assert!(knows_finish(&bin));
        assert!(!knows_finish(&d.path().join("absent")));
    }

    #[test]
    fn commande_finish_transmet_les_options() {
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
