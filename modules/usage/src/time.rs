//! Times: ISO 8601 reading (UTC or offset), display in local time, dates given by
//! the user (anything `date -d` understands).

use anyhow::{bail, Context, Result};
use std::process::Command;

/// (year, month, day) to days since 1970-01-01 (Gregorian calendar).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `2026-10-14T15:13:14.737Z`, `…+00:00`, `…+02:00`: UTC seconds (fraction ignored).
pub fn parse_iso(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !matches!(b[10], b'T' | b' ') {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let mut rest = &s[19..];
    if let Some(r) = rest.strip_prefix('.') {
        rest = r.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    let offset = match rest {
        "" | "Z" => 0,
        _ => {
            let sign = match rest.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let r = rest[1..].replace(':', "");
            if r.len() != 4 || !r.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let oh: i64 = r[..2].parse().ok()?;
            let om: i64 = r[2..].parse().ok()?;
            sign * (oh * 3600 + om * 60)
        }
    };
    let month_days = [
        31,
        if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
            29
        } else {
            28
        },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&mo)
        || d < 1
        || d > month_days[mo as usize - 1]
        || !(0..24).contains(&h)
        || !(0..60).contains(&mi)
        || !(0..=60).contains(&se)
    {
        return None;
    }
    Some(days_from_civil(y, mo as u32, d as u32) * 86_400 + h * 3600 + mi * 60 + se - offset)
}

/// `2026-10-14T15:13:14Z`
pub fn iso(secs: i64) -> String {
    poste_core::time::rfc3339(secs.max(0) as u64)
}

/// Local time according to `fmt` (strftime), through libc: the machine's time zone.
pub fn local(secs: i64, fmt: &str) -> String {
    let t = secs as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let fmt = std::ffi::CString::new(fmt).unwrap_or_default();
    let mut buf = [0u8; 64];
    // SAFETY: tm and buf are ours, sizes given to libc.
    let n = unsafe {
        if libc::localtime_r(&t, &mut tm).is_null() {
            return iso(secs);
        }
        libc::strftime(buf.as_mut_ptr().cast(), buf.len(), fmt.as_ptr(), &tm)
    };
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

/// `14/10 18:20`
pub fn short(secs: i64) -> String {
    local(secs, "%d/%m %H:%M")
}

/// A duration as `2 h 05` or `35 min`.
pub fn duration(secs: i64) -> String {
    let m = secs.max(0) / 60;
    if m >= 60 {
        format!("{} h {:02}", m / 60, m % 60)
    } else {
        format!("{m} min")
    }
}

/// A date given by the user: ISO 8601 directly, otherwise `date -d` ("3 days ago",
/// "yesterday 14:00", "2026-10-01"…).
pub fn parse_user(s: &str) -> Result<i64> {
    if let Some(t) = parse_iso(s) {
        return Ok(t);
    }
    let out = Command::new("date")
        .args(["-d", s, "+%s"])
        .output()
        .context("cannot run date")?;
    if !out.status.success() {
        bail!("unreadable date: {s}");
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .with_context(|| format!("unreadable date: {s}"))
}

pub fn now() -> i64 {
    poste_core::time::now_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_parsing() {
        assert_eq!(parse_iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso("2023-11-14T22:13:20Z"), Some(1_700_000_000));
        assert_eq!(
            parse_iso("2023-11-14T22:13:20.123456+00:00"),
            Some(1_700_000_000)
        );
        assert_eq!(parse_iso("2023-11-15T00:13:20+02:00"), Some(1_700_000_000));
        assert_eq!(parse_iso("2024-02-29T12:00:00Z"), Some(1_709_208_000));
        assert_eq!(parse_iso("demain"), None);
        assert_eq!(parse_iso(""), None);
        assert_eq!(parse_iso("2026-10-14T25:00:00Z"), None);
        assert_eq!(parse_iso("2026-02-31T00:00:00Z"), None);
        assert_eq!(parse_iso("2026-10-14T10:00:00+0é0"), None);
        assert_eq!(iso(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn durees() {
        assert_eq!(duration(35 * 60), "35 min");
        assert_eq!(duration(125 * 60), "2 h 05");
    }

    #[test]
    fn user_date() {
        assert_eq!(parse_user("2023-11-14T22:13:20Z").unwrap(), 1_700_000_000);
        assert_eq!(parse_user("@1700000000").unwrap(), 1_700_000_000);
        assert!(parse_user("not a date").is_err());
    }
}
