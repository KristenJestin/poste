//! Pochettes locales : `~/.cache/poste/music/covers/<id>.img`, téléchargées
//! une seule fois, en arrière-plan.

use crate::api::{pct_encode, Client};
use crate::db;
use crate::store::write_atomic;
use anyhow::{bail, Result};
use poste_core::Env;
use std::path::{Path, PathBuf};

/// Taille demandée au serveur.
const SIZE: u32 = 512;

/// Nom de fichier sûr pour un identifiant de pochette.
pub fn file_name(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}.img")
}

pub fn path(env: &Env, id: &str) -> PathBuf {
    db::cache_dir(env).join("covers").join(file_name(id))
}

/// Adresse `file://` d'un chemin absolu (caractères spéciaux encodés).
pub fn file_url(p: &Path) -> String {
    let s = p.to_string_lossy();
    let enc: Vec<String> = s.split('/').map(pct_encode).collect();
    format!("file://{}", enc.join("/"))
}

/// Télécharge la pochette `id` si elle n'est pas déjà là.
pub fn ensure(env: &Env, client: &Client, id: &str) -> Result<PathBuf> {
    let p = path(env, id);
    if p.is_file() {
        return Ok(p);
    }
    let bytes = client
        .fetch(
            "getCoverArt",
            &[("id", id.to_string()), ("size", SIZE.to_string())],
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if bytes.is_empty() {
        bail!("pochette vide");
    }
    write_atomic(&p, &bytes)?;
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_et_adresses() {
        assert_eq!(file_name("mf-ab12_65f"), "mf-ab12_65f.img");
        assert_eq!(file_name("../../etc/x y"), "______etc_x_y.img");
        let env = Env::with_home("/home/k");
        assert_eq!(
            path(&env, "al-1"),
            PathBuf::from("/home/k/.cache/poste/music/covers/al-1.img")
        );
        assert_eq!(
            file_url(Path::new("/home/k m/.cache/a.img")),
            "file:///home/k%20m/.cache/a.img"
        );
    }
}
