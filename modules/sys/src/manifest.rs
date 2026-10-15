//! `installed.json`: the list of what `poste sys setup` put in place.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub sha256: String,
    pub module: String,
    pub installed_at: String,
}

/// A line put in place by `poste sys setup` in a file without includes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineEntry {
    pub file: String,
    pub line: String,
    /// Owner module (`config` for `config/.poste/lines.toml`).
    pub module: String,
    pub installed_at: String,
}

/// A systemd user unit enabled by `poste sys setup`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UnitEntry {
    pub unit: String,
    /// Owner module.
    pub module: String,
    pub installed_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub files: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<LineEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub units: Vec<UnitEntry>,
}

impl Manifest {
    pub fn load(file: &Path) -> Result<Self> {
        match std::fs::read_to_string(file) {
            Ok(t) => serde_json::from_str(&t)
                .with_context(|| format!("{} is unreadable", file.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("reading {}", file.display())),
        }
    }

    /// Atomic write (temporary file then rename).
    pub fn save(&self, file: &Path) -> Result<()> {
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = file.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)? + "\n")?;
        std::fs::rename(&tmp, file)?;
        Ok(())
    }

    pub fn get(&self, path: &Path) -> Option<&Entry> {
        let p = path.to_string_lossy();
        self.files.iter().find(|e| e.path == p)
    }

    /// Adds or replaces the entry of a path.
    pub fn upsert(&mut self, entry: Entry) {
        match self.files.iter_mut().find(|e| e.path == entry.path) {
            Some(e) => *e = entry,
            None => self.files.push(entry),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(p: &str, h: &str) -> Entry {
        Entry {
            path: p.into(),
            sha256: h.into(),
            module: "m".into(),
            installed_at: "t".into(),
        }
    }

    #[test]
    fn round_trip_and_replacement() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("sub/installed.json");
        assert!(Manifest::load(&f).unwrap().files.is_empty());
        let mut m = Manifest::default();
        m.upsert(entry("/a", "1"));
        m.upsert(entry("/a", "2"));
        m.upsert(entry("/b", "3"));
        m.save(&f).unwrap();
        let m = Manifest::load(&f).unwrap();
        assert_eq!(m.files.len(), 2);
        assert_eq!(m.get(Path::new("/a")).unwrap().sha256, "2");
        assert!(m.get(Path::new("/c")).is_none());
    }
}
