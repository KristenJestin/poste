//! The index: its form on disk and the fuzzy search with neo_frizbee (fff's engine:
//! Smith-Waterman in SIMD, typo tolerant).
//!
//! On disk, text: a header line, then one line per entry, `d` (folder) or `f`, a tab, the path
//! (relative to the home folder when inside it, absolute otherwise). Each search reads the
//! file again and compares the paths where they lie in the buffer read: read in sequence, they
//! stay in the processor cache.

use anyhow::{bail, Result};
use neo_frizbee::{CaseMatching, Config, Matcher, Pattern, SortStrategy};
use std::path::{Path, PathBuf};

const MAGIC: &str = "poste-files-index 1";

/// A result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub path: String,
    pub is_dir: bool,
}

/// Header of the index: number of entries, walk duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meta {
    pub count: usize,
    pub scan_ms: u64,
}

impl Meta {
    pub fn parse(line: &str) -> Option<Self> {
        let mut f = line.strip_prefix(MAGIC)?.strip_prefix('\t')?.split('\t');
        Some(Self {
            count: f.next()?.parse().ok()?,
            scan_ms: f.next()?.parse().ok()?,
        })
    }
}

/// Text of the index for `entries`. Also returns the number of paths left out (not UTF-8,
/// or holding a line break).
pub fn serialize(entries: &[(PathBuf, bool)], home: &Path, scan_ms: u64) -> (String, usize) {
    let mut lines: Vec<(&str, bool)> = Vec::with_capacity(entries.len());
    let mut skipped = 0;
    for (p, d) in entries {
        let rel = p.strip_prefix(home).unwrap_or(p);
        match rel.to_str() {
            Some(s) if !s.contains('\n') && !s.is_empty() => lines.push((s, *d)),
            _ => skipped += 1,
        }
    }
    lines.sort_unstable();
    // A root inside another gives some entries twice.
    lines.dedup_by(|a, b| a.0 == b.0);
    let mut out = String::with_capacity(lines.iter().map(|(s, _)| s.len() + 3).sum::<usize>() + 64);
    out.push_str(&format!("{MAGIC}\t{}\t{scan_ms}\n", lines.len()));
    for (s, d) in lines {
        out.push(if d { 'd' } else { 'f' });
        out.push('\t');
        out.push_str(s);
        out.push('\n');
    }
    (out, skipped)
}

/// A path: `text[start..end]`.
#[derive(Debug)]
struct Span {
    start: u32,
    end: u32,
    /// Start of the name (after the last "/"), in `text`.
    name: u32,
    is_dir: bool,
}

#[derive(Debug)]
pub struct Index {
    text: String,
    spans: Vec<Span>,
    /// `$HOME/`, put back in front of relative paths.
    home: String,
}

/// Search threads: a short burst at each keystroke, never all the cores.
fn threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| (n.get() / 2).clamp(1, 8))
        .unwrap_or(1)
}

impl Index {
    /// Reads the text of an index.
    pub fn parse(text: String, home: &Path) -> Result<Self> {
        if text.len() > u32::MAX as usize {
            bail!("index too big");
        }
        let Some((head, _)) = text.split_once('\n') else {
            bail!("empty index");
        };
        let Some(meta) = Meta::parse(head) else {
            bail!("index in another format");
        };
        let mut spans = Vec::with_capacity(meta.count);
        let mut at = head.len() + 1;
        for line in text[at..].split_terminator('\n') {
            let b = line.as_bytes();
            if b.len() > 2 && b[1] == b'\t' {
                let start = at + 2;
                let end = at + line.len();
                spans.push(Span {
                    start: start as u32,
                    end: end as u32,
                    name: (start + line[2..].rfind('/').map_or(0, |i| i + 1)) as u32,
                    is_dir: b[0] == b'd',
                });
            }
            at += line.len() + 1;
        }
        let mut home = home.to_string_lossy().into_owned();
        if !home.ends_with('/') {
            home.push('/');
        }
        Ok(Self { text, spans, home })
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    fn path(&self, s: &Span) -> &str {
        &self.text[s.start as usize..s.end as usize]
    }

    fn name(&self, s: &Span) -> &str {
        &self.text[s.name as usize..s.end as usize]
    }

    /// The query without the home folder in front of its words: the index keeps the paths
    /// inside it relative, so a pasted `/home/<user>/x` or `~/x` must look for `x`.
    fn strip_home(&self, query: &str) -> String {
        let bare = self.home.trim_end_matches('/');
        query
            .split_whitespace()
            .filter(|w| *w != "~" && *w != bare && *w != self.home)
            .map(|w| {
                w.strip_prefix(self.home.as_str())
                    .or_else(|| w.strip_prefix("~/"))
                    .unwrap_or(w)
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The `limit` best results. Each word of the query must be found, letters in order, in
    /// the path; a matching file name comes before a matching path, and at equal score the
    /// shortest wins. Few results and a word of 6 letters or more: try again allowing one typo
    /// in each such word, the other words still required.
    pub fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
        let query = self.strip_home(query);
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }
        let patterns = Pattern::parse_query(&query);
        if patterns.is_empty() {
            return Vec::new();
        }
        // Case always ignored (no smart case: "Huk19" must find "HUk19e…"); the same case
        // still adds to the score.
        let config = Config::default()
            .sort(SortStrategy::Unsorted)
            .casing(CaseMatching::Ignore);
        let threads = threads();
        let len = |i: u32| {
            let s = &self.spans[i as usize];
            s.end - s.start
        };
        let key = |&(s, i): &(u32, u32)| (std::cmp::Reverse(s), len(i), i);
        let top = |found: &mut Vec<(u32, u32)>, n: usize| {
            if found.len() > n {
                found.select_nth_unstable_by_key(n - 1, key);
                found.truncate(n);
            }
        };

        let mut matcher = Matcher::from_patterns(&patterns, &config);
        let paths: Vec<&str> = self.spans.iter().map(|s| self.path(s)).collect();
        // (score, rank in the index)
        let mut found: Vec<(u32, u32)> = matcher
            .match_list_parallel(&paths, threads)
            .into_iter()
            .map(|m| (u32::from(m.score), m.index))
            .collect();
        // Name bonus, on the best candidates only: the alignment on the path already lands
        // on the name when it is there in one piece.
        top(&mut found, NAME_CANDIDATES);
        let names: Vec<&str> = found
            .iter()
            .map(|&(_, i)| self.name(&self.spans[i as usize]))
            .collect();
        for m in matcher.match_list_parallel(&names, threads) {
            let f = &mut found[m.index as usize];
            f.0 = f.0.max(name_score(m.score, m.exact));
        }

        // Second pass allowing one typo in each long word; every word of the query must still
        // be in the path, as in the first pass.
        let long = |p: &Pattern| p.needle.chars().count() >= 6;
        if found.len() < limit && patterns.iter().any(long) {
            let typos: Vec<Pattern> = patterns
                .into_iter()
                .map(|p| if long(&p) { p.max_typos(Some(1)) } else { p })
                .collect();
            let known: std::collections::HashSet<u32> = found.iter().map(|f| f.1).collect();
            let mut matcher = Matcher::from_patterns(&typos, &config);
            for m in matcher.match_list_parallel(&paths, threads) {
                if !known.contains(&m.index) {
                    // A typo costs: these results come after the others at equal score.
                    found.push((u32::from(m.score) / 2, m.index));
                }
            }
        }
        top(&mut found, limit);
        found.sort_unstable_by_key(key);
        found
            .into_iter()
            .map(|(_, i)| {
                let s = &self.spans[i as usize];
                let p = self.path(s);
                Hit {
                    path: if p.starts_with('/') {
                        p.to_string()
                    } else {
                        format!("{}{p}", self.home)
                    },
                    is_dir: s.is_dir,
                }
            })
            .collect()
    }
}

/// Candidates (the best placed on the path) whose name is compared too.
const NAME_CANDIDATES: usize = 2000;

/// Score of a matching name: one and a half times a path's, twice when exact.
fn name_score(score: u16, exact: bool) -> u32 {
    let bonus = if exact { 2 } else { 1 };
    u32::from(score) * (2 + bonus) / 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::tests::{rules, sample_tree};
    use crate::rules::walk;

    fn index_of(paths: &[(&str, bool)]) -> Index {
        let entries: Vec<(PathBuf, bool)> =
            paths.iter().map(|(p, d)| (PathBuf::from(p), *d)).collect();
        let (text, skipped) = serialize(&entries, Path::new("/h"), 7);
        assert_eq!(skipped, 0);
        Index::parse(text, Path::new("/h")).unwrap()
    }

    #[test]
    fn form_on_disk() {
        let entries = vec![
            (PathBuf::from("/h/b.txt"), false),
            (PathBuf::from("/h/a"), true),
            (PathBuf::from("/srv/x"), false),
            (PathBuf::from("/h/line\nbreak"), false),
        ];
        let (text, skipped) = serialize(&entries, Path::new("/h"), 12);
        assert_eq!(skipped, 1);
        assert_eq!(
            Meta::parse(text.lines().next().unwrap()),
            Some(Meta {
                count: 3,
                scan_ms: 12
            })
        );
        assert_eq!(
            text,
            "poste-files-index 1\t3\t12\nf\t/srv/x\nd\ta\nf\tb.txt\n"
        );
        let ix = Index::parse(text, Path::new("/h/")).unwrap();
        assert_eq!(ix.len(), 3);
        assert_eq!(
            ix.search("srv", 5),
            [Hit {
                path: "/srv/x".into(),
                is_dir: false
            }]
        );
        assert!(Index::parse("something else\n".into(), Path::new("/h")).is_err());
        assert!(Index::parse(String::new(), Path::new("/h")).is_err());
    }

    #[test]
    fn fuzzy_search_name_before_path() {
        let ix = index_of(&[
            ("/h/Projects/poste/config/hypr/bindings.lua", false),
            ("/h/Projects/bind/README.md", false),
            ("/h/Documents/invoice-2026.pdf", false),
            ("/h/Documents", true),
            ("/srv/data/bindings.lua", false),
        ]);
        let hits = ix.search("bindings", 10);
        assert_eq!(hits[0].path, "/srv/data/bindings.lua");
        assert_eq!(hits[1].path, "/h/Projects/poste/config/hypr/bindings.lua");
        // Several words: all must be in the path.
        let hits = ix.search("poste bind", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].path, "/h/Projects/poste/config/hypr/bindings.lua");
        // Typo allowed on a long word.
        let hits = ix.search("invocie", 10);
        // The typo pass still requires the other words.
        assert!(ix.search("poste invocie", 10).is_empty());
        assert_eq!(ix.search("Documents invocie", 10).len(), 1);
        assert_eq!(hits[0].path, "/h/Documents/invoice-2026.pdf");
        // The home folder prefix does not count: "h" does not find everything.
        assert!(ix
            .search("h", 10)
            .iter()
            .all(|h| !h.path.starts_with("/h/Doc")));
        assert!(ix.search("   ", 10).is_empty());
        // A pasted absolute path, or one from "~", finds what its relative form finds.
        for q in ["/h/Documents/invoice", "~/Documents/invoice", "/h Documents/invoice"] {
            assert_eq!(
                ix.search(q, 10)[0].path,
                "/h/Documents/invoice-2026.pdf",
                "{q}"
            );
        }
        assert!(ix.search("/h/", 10).is_empty());
        assert!(ix.search("~", 10).is_empty());
        // Mixed case: ignored.
        assert_eq!(
            ix.search("InVoice", 10)[0].path,
            "/h/Documents/invoice-2026.pdf"
        );
        assert_eq!(
            ix.search("Documents", 1)[0],
            Hit {
                path: "/h/Documents".into(),
                is_dir: true
            }
        );
    }

    #[test]
    fn search_on_a_fake_home() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("home");
        sample_tree(&root);
        let (text, _) = serialize(
            &walk(&rules(&root, ""), std::slice::from_ref(&root), 2),
            &root,
            0,
        );
        let ix = Index::parse(text, &root).unwrap();
        let hits = ix.search("player", 5);
        assert!(hits[0].path.ends_with("Projects/game/Assets/Player.cs"));
        assert!(ix.search("left-pad", 5).is_empty());
        assert!(ix.search("ShaderCache", 5).is_empty());
        let hits = ix.search("pancake", 5);
        assert!(hits[0].path.ends_with("notes/pancake-recipe.md"));
        // Ignored by the repository's .gitignore, but found.
        assert!(ix.search("bundle", 5)[0]
            .path
            .ends_with("site/out/bundle.js"));
    }
}
