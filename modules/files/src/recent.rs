//! Recent files, shown when the extension opens with nothing typed: the ones opened from the
//! launcher (`poste files open`, kept in `$XDG_STATE_HOME/poste/files-recent`) merged with
//! the desktop's own list, `$XDG_DATA_HOME/recently-used.xbel` (freedesktop, filled by GTK
//! applications). Newest first, only what still exists.

use crate::index::Hit;
use anyhow::{Context, Result};
use poste_core::Env;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Lines kept in our own list.
const KEEP: usize = 100;

fn own_path(env: &Env) -> PathBuf {
    env.state_dir().join("files-recent")
}

fn xbel_path(env: &Env) -> PathBuf {
    env.data_home.join("recently-used.xbel")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Puts `path` on top of our list (`<unix seconds>\t<path>` per line, newest first).
pub fn record(env: &Env, path: &Path) -> Result<()> {
    let Some(p) = path.to_str().filter(|p| !p.contains('\n')) else {
        return Ok(());
    };
    let file = own_path(env);
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    let mut out = format!("{}\t{p}\n", now());
    for line in old
        .lines()
        .filter(|l| l.split_once('\t').is_some_and(|(_, q)| q != p))
        .take(KEEP - 1)
    {
        out.push_str(line);
        out.push('\n');
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&file, out).with_context(|| format!("writing {}", file.display()))
}

/// Our list: (unix seconds, path).
fn parse_own(text: &str) -> Vec<(u64, String)> {
    text.lines()
        .filter_map(|l| {
            let (t, p) = l.split_once('\t')?;
            Some((t.parse().ok()?, p.to_string()))
        })
        .collect()
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// `2026-10-15T11:26:40.123456Z` (UTC, as written in xbel) to unix seconds.
fn parse_time(s: &str) -> Option<u64> {
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let days = days_from_civil(n(0..4)?, n(5..7)?, n(8..10)?);
    let secs = days * 86400 + n(11..13)? * 3600 + n(14..16)? * 60 + n(17..19)?;
    u64::try_from(secs).ok()
}

/// `%XX` sequences decoded; `None` if the result is not UTF-8.
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The value of attribute `name` in the tag text `tag`.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let start = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let len = tag[start..].find('"')?;
    Some(&tag[start..start + len])
}

/// The local files of a `recently-used.xbel`: (unix seconds, path).
fn parse_xbel(text: &str) -> Vec<(u64, String)> {
    let mut out = Vec::new();
    for chunk in text.split("<bookmark ").skip(1) {
        let tag = &chunk[..chunk.find('>').unwrap_or(chunk.len())];
        let tag = format!(" {tag}");
        let Some(path) = attr(&tag, "href")
            .and_then(|h| h.strip_prefix("file://"))
            .and_then(percent_decode)
            .map(|p| p.replace("&amp;", "&"))
        else {
            continue;
        };
        let time = ["visited", "modified", "added"]
            .iter()
            .filter_map(|a| attr(&tag, a).and_then(parse_time))
            .max()
            .unwrap_or_default();
        out.push((time, path));
    }
    out
}

/// Merged recent files, newest first, at most `limit`, only those that still exist.
pub fn list(env: &Env, limit: usize) -> Vec<Hit> {
    let mut best: HashMap<String, u64> = HashMap::new();
    let own = std::fs::read_to_string(own_path(env)).unwrap_or_default();
    let xbel = std::fs::read_to_string(xbel_path(env)).unwrap_or_default();
    for (t, p) in parse_own(&own).into_iter().chain(parse_xbel(&xbel)) {
        let e = best.entry(p).or_default();
        *e = (*e).max(t);
    }
    let mut all: Vec<(u64, String)> = best.into_iter().map(|(p, t)| (t, p)).collect();
    all.sort_unstable_by(|a, b| b.cmp(a));
    all.into_iter()
        .filter_map(|(_, path)| {
            let meta = std::fs::metadata(&path).ok()?;
            Some(Hit {
                is_dir: meta.is_dir(),
                path,
            })
        })
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn xbel_dates_and_escapes() {
        assert_eq!(parse_time("1970-01-02T00:00:01Z"), Some(86401));
        assert_eq!(
            parse_time("2026-10-15T11:26:40.123456Z"),
            Some(1_792_063_600)
        );
        assert_eq!(parse_time("garbage"), None);
        let text = r#"<?xml version="1.0"?>
<xbel version="1.0">
  <bookmark href="file:///h/My%20docs/caf%C3%A9.txt" added="2026-10-01T10:00:00Z" modified="2026-10-02T10:00:00Z" visited="2026-10-03T10:00:00Z">
  </bookmark>
  <bookmark href="https://example.com/" added="2026-10-01T10:00:00Z" modified="2026-10-01T10:00:00Z" visited="2026-10-01T10:00:00Z"/>
  <bookmark href="file:///h/a&amp;b.pdf" added="2026-09-01T10:00:00Z" modified="2026-09-01T10:00:00Z" visited="2026-09-01T10:00:00Z"/>
</xbel>"#;
        let v = parse_xbel(text);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].1, "/h/My docs/café.txt");
        assert_eq!(v[0].0, parse_time("2026-10-03T10:00:00Z").unwrap());
        assert_eq!(v[1].1, "/h/a&b.pdf");
    }

    #[test]
    fn merged_newest_first_existing_only() {
        let t = tempfile::tempdir().unwrap();
        let home = t.path().join("home");
        let env = Env::with_home(&home);
        let a = home.join("a.txt");
        let b = home.join("docs");
        fs::create_dir_all(&b).unwrap();
        fs::write(&a, "x").unwrap();
        fs::create_dir_all(xbel_path(&env).parent().unwrap()).unwrap();
        fs::write(
            xbel_path(&env),
            format!(
                "<xbel><bookmark href=\"file://{}\" visited=\"2001-01-01T00:00:00Z\"/>\
                 <bookmark href=\"file://{}/gone.txt\" visited=\"2030-01-01T00:00:00Z\"/></xbel>",
                a.display(),
                home.display()
            ),
        )
        .unwrap();
        assert!(list(&env, 10)
            .iter()
            .map(|h| &h.path)
            .eq([&a.display().to_string()]));
        record(&env, &b).unwrap();
        record(&env, &a).unwrap();
        record(&env, &b).unwrap();
        let v = list(&env, 10);
        assert_eq!(v.len(), 2, "{v:?}");
        assert_eq!(v[0].path, b.display().to_string());
        assert!(v[0].is_dir);
        assert_eq!(v[1].path, a.display().to_string());
        assert_eq!(list(&env, 1).len(), 1);
        // Recorded once, even after being opened twice.
        let own = fs::read_to_string(own_path(&env)).unwrap();
        assert_eq!(own.lines().count(), 2);
    }
}
