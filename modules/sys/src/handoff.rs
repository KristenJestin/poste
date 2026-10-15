//! Relais vers le `poste-sys` de la nouvelle racine.
//!
//! `sync` et `use` font pointer `current` sur une autre racine (version publiée, clone tiré)
//! puis lancent `setup`. Ce `setup` doit être celui de la nouvelle racine : une nouveauté de
//! `poste sys` (une sorte d'unité systemd, une règle de pose) arrive avec la version qui s'en
//! sert, et l'ancien programme la raterait. Avant `setup`, si le `poste-sys` de la nouvelle
//! racine diffère de celui qui tourne, on lui passe la main (`exec`) par la commande cachée
//! `finish` : il lance `setup` depuis `current`, élague les anciennes versions si demandé,
//! puis recharge l'île et Hyprland.
//!
//! `finish` est un contrat entre versions (une ancienne passe la main à une nouvelle) : ses
//! options ne changent pas, on en ajoute seulement.

use crate::reload::Need;
use crate::util::sha256_file;
use anyhow::Result;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Posée sur le programme relayé : il ne repasse jamais la main.
const RELAYED: &str = "POSTE_SYS_RELAYED";

/// Le `poste-sys` d'une racine (construit par `cargo` dans le clone, livré dans une version).
pub fn binary(root: &Path) -> PathBuf {
    root.join("target/release/poste-sys")
}

/// Le programme qui tourne. Aucun relais dans les tests : le binaire de test n'est pas
/// `poste-sys`.
pub fn running() -> Option<PathBuf> {
    if cfg!(test) {
        None
    } else {
        std::env::current_exe().ok()
    }
}

/// Le `poste-sys` de `root` à qui passer la main, s'il diffère de `running`.
pub fn target(root: &Path, running: Option<&Path>) -> Result<Option<PathBuf>> {
    let Some(running) = running else {
        return Ok(None);
    };
    let bin = binary(root);
    if std::env::var_os(RELAYED).is_some() || !bin.is_file() {
        return Ok(None);
    }
    // Le programme qui tourne peut avoir été remplacé entre-temps (illisible) : on passe la main.
    let same = sha256_file(running).is_ok_and(|h| sha256_file(&bin).is_ok_and(|b| b == h));
    Ok((!same).then_some(bin))
}

/// Avant `setup` depuis `root` : le `poste-sys` à qui passer la main, s'il y en a un. Le clone se
/// construit d'abord (son `poste-sys` est celui du code tiré).
pub fn prepare(root: &Path, skip_build: bool) -> Result<Option<PathBuf>> {
    let Some(running) = running() else {
        return Ok(None);
    };
    if !skip_build && root.join("Cargo.toml").is_file() {
        crate::setup::build(root)?;
    }
    target(root, Some(&running))
}

/// Passe la main à `bin` (`finish`) ; ne revient qu'en cas d'échec du lancement.
pub fn relay(bin: &Path, need: &Need, yes: bool, prune: bool) -> anyhow::Error {
    println!(
        "\nNouveau poste-sys ({}) : c'est lui qui fait la suite.",
        bin.display()
    );
    let err = finish_command(bin, need, yes, prune).exec();
    anyhow::Error::new(err).context(format!("impossible de lancer {}", bin.display()))
}

fn finish_command(bin: &Path, need: &Need, yes: bool, prune: bool) -> Command {
    let mut c = Command::new(bin);
    c.arg("finish").env(RELAYED, "1");
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
        // Pas de poste-sys dans la racine, ou aucun programme connu : on reste.
        assert_eq!(target(&root, Some(&running)).unwrap(), None);
        fs::create_dir_all(binary(&root).parent().unwrap()).unwrap();
        fs::write(binary(&root), "ancien").unwrap();
        assert_eq!(target(&root, None).unwrap(), None);
        // Le même : on reste ; un autre : on passe la main.
        assert_eq!(target(&root, Some(&running)).unwrap(), None);
        fs::write(binary(&root), "nouveau").unwrap();
        assert_eq!(target(&root, Some(&running)).unwrap(), Some(binary(&root)));
    }

    #[test]
    fn commande_finish_transmet_les_options() {
        let need = Need {
            shell: true,
            hypr: false,
        };
        let c = finish_command(Path::new("/x/poste-sys"), &need, true, true);
        let args: Vec<_> = c
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["finish", "--yes", "--prune", "--reload-shell"]);
        assert!(c.get_envs().any(|(k, v)| k == RELAYED && v.is_some()));
    }
}
