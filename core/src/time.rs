//! Dates UTC sans dépendance externe.

use std::time::{SystemTime, UNIX_EPOCH};

/// Jours depuis 1970-01-01 vers (année, mois, jour) du calendrier grégorien.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn parts(secs: u64) -> (i64, u32, u32, u64, u64, u64) {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    (y, m, d, rest / 3600, (rest % 3600) / 60, rest % 60)
}

/// `2026-10-08T12:00:00Z`
pub fn rfc3339(secs: u64) -> String {
    let (y, m, d, hh, mm, ss) = parts(secs);
    format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

/// `20261008-120000` : utilisable dans un nom de fichier.
pub fn compact(secs: u64) -> String {
    let (y, m, d, hh, mm, ss) = parts(secs);
    format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}")
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn now_rfc3339() -> String {
    rfc3339(now_secs())
}

pub fn now_compact() -> String {
    compact(now_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_connues() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(compact(1_709_208_000), "20240229-120000");
    }
}
