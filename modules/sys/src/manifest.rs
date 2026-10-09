//! `installed.json` : la liste de ce que `poste sys setup` a posé.

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

/// Une ligne posée par `poste sys setup` dans un fichier sans inclusion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineEntry {
    pub file: String,
    pub line: String,
    /// Module propriétaire (`config` pour `config/.poste/lines.toml`).
    pub module: String,
    pub installed_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub files: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<LineEntry>,
}

impl Manifest {
    pub fn load(file: &Path) -> Result<Self> {
        match std::fs::read_to_string(file) {
            Ok(t) => {
                serde_json::from_str(&t).with_context(|| format!("{} illisible", file.display()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("lecture de {}", file.display())),
        }
    }

    /// Écriture atomique (fichier temporaire puis renommage).
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

    /// Ajoute ou remplace l'entrée d'un chemin.
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
    fn aller_retour_et_remplacement() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("sous/installed.json");
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
