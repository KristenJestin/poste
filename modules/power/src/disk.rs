//! Occupation des disques (`statvfs`).

use std::ffi::CString;

#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub path: String,
    /// Occupation en % comme `df` (utilisé / (utilisé + disponible)).
    pub used_pct: f64,
    pub free_bytes: u64,
    pub total_bytes: u64,
    /// Empreinte (blocs, blocs libres) pour reconnaître deux chemins sur le même disque.
    pub fingerprint: (u64, u64, u64),
}

/// `statvfs` d'un chemin ; `None` s'il n'existe pas ou n'est pas lisible.
pub fn usage(path: &str) -> Option<Usage> {
    let c = CString::new(path).ok()?;
    // SAFETY : `st` est un tampon local valide ; `statvfs` ne lit que le chemin C terminé par NUL.
    let st = unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut st) != 0 {
            return None;
        }
        st
    };
    let bs = st.f_frsize as u64;
    let blocks = st.f_blocks as u64;
    let bfree = st.f_bfree as u64;
    let bavail = st.f_bavail as u64;
    Some(from_blocks(path, bs, blocks, bfree, bavail))
}

pub fn from_blocks(path: &str, bs: u64, blocks: u64, bfree: u64, bavail: u64) -> Usage {
    let used = blocks.saturating_sub(bfree);
    let denom = used + bavail;
    Usage {
        path: path.to_string(),
        used_pct: if denom == 0 {
            0.0
        } else {
            used as f64 * 100.0 / denom as f64
        },
        free_bytes: bavail * bs,
        total_bytes: blocks * bs,
        fingerprint: (blocks, bfree, bavail),
    }
}

/// Usages des chemins demandés, sans les chemins absents ni ceux qui partagent le disque
/// d'un chemin déjà listé (`/home` en sous-volume de `/`).
pub fn usages(paths: &[String]) -> Vec<Usage> {
    let mut out: Vec<Usage> = Vec::new();
    for p in paths {
        let Some(u) = usage(p) else { continue };
        if out.iter().any(|o| same_disk(o, &u)) {
            continue;
        }
        out.push(u);
    }
    out
}

fn same_disk(a: &Usage, b: &Usage) -> bool {
    // Mêmes blocs total et libres au même instant : le même disque.
    a.fingerprint.0 == b.fingerprint.0 && a.fingerprint.1 == b.fingerprint.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pourcentage_comme_df() {
        // 1000 blocs, 100 libres dont 50 pour les utilisateurs : utilisé 900, dispo 50 -> 94,7 %.
        let u = from_blocks("/", 4096, 1000, 100, 50);
        assert!((u.used_pct - 94.736).abs() < 0.01);
        assert_eq!(u.free_bytes, 50 * 4096);
        assert_eq!(from_blocks("/", 4096, 0, 0, 0).used_pct, 0.0);
    }

    #[test]
    fn vrai_systeme_de_fichiers() {
        let u = usage("/").expect("/ existe");
        assert!(u.total_bytes > 0 && u.used_pct >= 0.0 && u.used_pct <= 100.0);
        assert!(usage("/n'existe/pas").is_none());
        // Le même chemin donné deux fois ne compte qu'une fois.
        assert_eq!(
            usages(&["/".into(), "/".into(), "/absent-xyz".into()]).len(),
            1
        );
    }
}
