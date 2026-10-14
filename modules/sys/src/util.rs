//! Petits utilitaires partagés : dépôt, empreintes.

use anyhow::{bail, Context, Result};
use poste_core::{Config, Env};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

/// Emplacement du clone du dépôt, lu dans config.toml (clé `repo`, écrite par `./setup`). Pour
/// ce qui travaille sur le dépôt lui-même (git, import) ; ce qui s'installe se lit dans la
/// racine (`crate::versions::root`).
pub fn clone(env: &Env, cfg: &Config) -> Result<PathBuf> {
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

/// Sauvegarde ce qui se trouve à `dest` dans `backup/` sous un nom horodaté et renvoie
/// ce chemin. Un fichier est copié, un lien est recréé tel quel, un dossier est déplacé
/// (jamais supprimé sans copie complète). Ne touche pas à `dest` sauf pour un dossier.
pub fn backup(env: &Env, dest: &Path, stamp: &str) -> Result<PathBuf> {
    let dir = env.backup_dir();
    std::fs::create_dir_all(&dir)?;
    let rel = dest.strip_prefix(&env.home).unwrap_or(dest);
    let flat: String = rel
        .to_string_lossy()
        .trim_start_matches('/')
        .chars()
        .map(|c| if c == '/' { '_' } else { c })
        .collect();
    let mut bak = dir.join(format!("{flat}.{stamp}.bak"));
    let mut n = 1;
    while std::fs::symlink_metadata(&bak).is_ok() {
        bak = dir.join(format!("{flat}.{stamp}.{n}.bak"));
        n += 1;
    }
    let md = std::fs::symlink_metadata(dest)
        .with_context(|| format!("sauvegarde de {}", dest.display()))?;
    if md.file_type().is_symlink() {
        symlink(std::fs::read_link(dest)?, &bak)?;
    } else if md.is_dir() {
        if std::fs::rename(dest, &bak).is_err() {
            // Autre système de fichiers : copie complète, puis seulement ensuite retrait.
            copy_tree(dest, &bak)?;
            std::fs::remove_dir_all(dest)?;
        }
    } else {
        std::fs::copy(dest, &bak).with_context(|| format!("sauvegarde de {}", dest.display()))?;
    }
    Ok(bak)
}

/// Copie récursive qui garde les liens tels quels.
pub fn copy_tree(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let (s, d) = (e.path(), dst.join(e.file_name()));
        let ft = e.file_type()?;
        if ft.is_symlink() {
            symlink(std::fs::read_link(&s)?, &d)?;
        } else if ft.is_dir() {
            copy_tree(&s, &d)?;
        } else {
            std::fs::copy(&s, &d).with_context(|| format!("copie de {}", s.display()))?;
        }
    }
    Ok(())
}

/// Pose un lien symbolique en remplaçant atomiquement ce qui est à `dest`
/// (fichier ou lien ; un dossier doit avoir été écarté avant).
pub fn replace_with_symlink(dest: &Path, target: &Path) -> Result<()> {
    let parent = dest.parent().context("destination sans dossier parent")?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".poste-tmp-{}-lien", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    symlink(target, &tmp).with_context(|| format!("création du lien {}", tmp.display()))?;
    if let Err(e) = std::fs::rename(&tmp, dest) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("pose du lien {}", dest.display()));
    }
    Ok(())
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
