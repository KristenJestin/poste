//! Petits utilitaires partagés : dépôt, empreintes.

use anyhow::{bail, Context, Result};
use poste_core::{Config, Env};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Emplacement du dépôt, lu dans config.toml (clé `repo`, écrite par `./setup`).
pub fn repo(env: &Env, cfg: &Config) -> Result<PathBuf> {
    let Some(repo) = cfg.repo_path(env) else {
        bail!(
            "dépôt inconnu : la clé `repo` manque dans {}. Lancez ./setup depuis le dépôt.",
            env.config_file().display()
        );
    };
    if !repo.join("Cargo.toml").is_file() {
        bail!(
            "le dépôt {} n'existe plus ou n'est pas un espace de travail Cargo",
            repo.display()
        );
    }
    Ok(repo)
}

/// Empreinte sha256 (hexadécimal) d'un fichier.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut f =
        std::fs::File::open(path).with_context(|| format!("lecture de {}", path.display()))?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// Début d'une empreinte, pour l'affichage.
pub fn short(hash: &str) -> &str {
    &hash[..hash.len().min(8)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empreinte_connue() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("f");
        std::fs::write(&p, "abc").unwrap();
        assert_eq!(
            sha256_file(&p).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(short("ba7816bf8f01"), "ba7816bf");
    }
}
