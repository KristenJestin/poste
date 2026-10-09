//! Règles d'exclusion de `config/.poste/ignore` : chemins que `add` refuse d'ajouter
//! (secrets, caches, fichiers régénérés).
//!
//! Syntaxe : un motif par ligne, relatif à `~`, `#` pour un commentaire. `*` et `?`
//! ne franchissent pas un `/`, `**` couvre zéro dossier ou plus. Un motif qui correspond
//! à un dossier couvre tout ce qu'il contient.

use anyhow::{Context, Result};
use std::path::Path;

/// Règles toujours actives, même sans fichier : le strict minimum pour les secrets.
const BUILTIN: &[&str] = &[
    ".ssh/**",
    ".gnupg/**",
    ".npmrc",
    ".netrc",
    ".claude/.credentials.json",
    "**/.env",
    "**/.env.*",
];

#[derive(Debug, Clone, Default)]
pub struct Ignore {
    patterns: Vec<String>,
}

impl Ignore {
    /// Règles intégrées plus celles du fichier (absent : seulement les intégrées).
    pub fn load(file: &Path) -> Result<Self> {
        let mut patterns: Vec<String> = BUILTIN.iter().map(|s| (*s).to_string()).collect();
        match std::fs::read_to_string(file) {
            Ok(t) => patterns.extend(parse(&t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("lecture de {}", file.display())),
        }
        Ok(Self { patterns })
    }

    /// Première règle qui correspond au chemin (relatif à `~`, séparé par `/`).
    pub fn matching(&self, rel: &str) -> Option<&str> {
        self.patterns
            .iter()
            .find(|p| matches(p, rel))
            .map(String::as_str)
    }
}

pub fn parse(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// Le motif couvre-t-il le chemin ou l'un de ses dossiers parents ?
pub fn matches(pattern: &str, rel: &str) -> bool {
    let pat: Vec<&str> = pattern.trim_matches('/').split('/').collect();
    let segs: Vec<&str> = rel.trim_matches('/').split('/').collect();
    (1..=segs.len()).any(|k| seg_match(&pat, &segs[..k]))
}

fn seg_match(pat: &[&str], segs: &[&str]) -> bool {
    match pat.split_first() {
        None => segs.is_empty(),
        Some((&"**", rest)) => (0..=segs.len()).any(|i| seg_match(rest, &segs[i..])),
        Some((first, rest)) => {
            !segs.is_empty() && wild(first, segs[0]) && seg_match(rest, &segs[1..])
        }
    }
}

/// Correspondance d'un nom avec `*` et `?`.
fn wild(pat: &str, text: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let mut ok = vec![vec![false; t.len() + 1]; p.len() + 1];
    ok[0][0] = true;
    for i in 1..=p.len() {
        if p[i - 1] == '*' {
            ok[i][0] = ok[i - 1][0];
        }
        for j in 1..=t.len() {
            ok[i][j] = match p[i - 1] {
                '*' => ok[i - 1][j] || ok[i][j - 1],
                '?' => ok[i - 1][j - 1],
                c => ok[i - 1][j - 1] && c == t[j - 1],
            };
        }
    }
    ok[p.len()][t.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motifs() {
        assert!(matches(".ssh/**", ".ssh/id_ed25519"));
        assert!(matches(".ssh/**", ".ssh"));
        assert!(!matches(".ssh/**", ".sshx/a"));
        assert!(matches("**/.env*", ".config/app/.env.local"));
        assert!(matches("**/.env*", ".env"));
        assert!(!matches("**/.env*", ".config/app/env"));
        assert!(matches(".npmrc", ".npmrc"));
        assert!(matches(
            ".config/omarchy/current",
            ".config/omarchy/current/theme/x"
        ));
        assert!(matches("**/__pycache__/**", "a/b/__pycache__/c.pyc"));
        assert!(matches("**/*.bak.*", "a/f.bak.2026"));
        assert!(!matches("**/*.bak", "a/f.bak.2026"));
    }

    #[test]
    fn analyse_et_regles_integrees() {
        assert_eq!(parse("# note\n\n .a/** \nb\n"), vec![".a/**", "b"]);
        let d = tempfile::tempdir().unwrap();
        let ig = Ignore::load(&d.path().join("absent")).unwrap();
        assert_eq!(ig.matching(".ssh/config"), Some(".ssh/**"));
        assert!(ig.matching(".config/hypr/bindings.lua").is_none());
        let f = d.path().join("ignore");
        std::fs::write(&f, ".cache/**\n").unwrap();
        let ig = Ignore::load(&f).unwrap();
        assert!(ig.matching(".cache/x").is_some());
    }
}
