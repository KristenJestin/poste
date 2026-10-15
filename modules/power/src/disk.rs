//! Disk usage (`statvfs`).

use std::ffi::CString;

#[derive(Debug, Clone, PartialEq)]
pub struct Usage {
    pub path: String,
    /// Usage in % like `df` (used / (used + available)).
    pub used_pct: f64,
    pub free_bytes: u64,
    pub total_bytes: u64,
    /// Fingerprint (blocks, free blocks) to recognize two paths on the same disk.
    pub fingerprint: (u64, u64, u64),
}

/// `statvfs` of a path; `None` if it does not exist or is not readable.
pub fn usage(path: &str) -> Option<Usage> {
    let c = CString::new(path).ok()?;
    // SAFETY: `st` is a valid local buffer; `statvfs` only reads the NUL-terminated C path.
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

/// Usages of the requested paths, without the absent paths or those that share the disk
/// of a path already listed (`/home` as a sub-volume of `/`).
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
    // Same total and free blocks at the same instant: the same disk.
    a.fingerprint.0 == b.fingerprint.0 && a.fingerprint.1 == b.fingerprint.1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentage_like_df() {
        // 1000 blocks, 100 free of which 50 for users: used 900, available 50 -> 94.7 %.
        let u = from_blocks("/", 4096, 1000, 100, 50);
        assert!((u.used_pct - 94.736).abs() < 0.01);
        assert_eq!(u.free_bytes, 50 * 4096);
        assert_eq!(from_blocks("/", 4096, 0, 0, 0).used_pct, 0.0);
    }

    #[test]
    fn real_filesystem() {
        let u = usage("/").expect("/ exists");
        assert!(u.total_bytes > 0 && u.used_pct >= 0.0 && u.used_pct <= 100.0);
        assert!(usage("/does/not/exist").is_none());
        // The same path given twice counts only once.
        assert_eq!(
            usages(&["/".into(), "/".into(), "/absent-xyz".into()]).len(),
            1
        );
    }
}
