//! Ce que le dépôt veut installer, et ce qu'il faut faire pour y arriver.

use crate::manifest::Manifest;
use crate::util::sha256_file;
use anyhow::{bail, Context, Result};
use poste_core::{Card, Env};
use std::path::{Path, PathBuf};

/// Un fichier à poser : source dans le dépôt, destination dans le système.
#[derive(Debug, Clone)]
pub struct Item {
    /// Module propriétaire (`poste` pour l'aiguillage).
    pub module: String,
    pub src: PathBuf,
    pub dest: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// La destination n'existe pas.
    Create,
    /// Posé par nous, à remplacer (`by_hand` : modifié à la main depuis).
    Update { by_hand: bool },
    /// Déjà identique et suivi.
    Unchanged,
    /// Déjà identique mais pas encore suivi : on l'enregistre sans rien écrire.
    Adopt,
    /// Existe, diffère, et ne vient pas de nous : on n'y touche pas.
    Conflict,
}

/// Un module du dépôt : `modules/<dir>/` contenant un `module.toml`.
#[derive(Debug, Clone)]
pub struct RepoModule {
    pub name: String,
    pub dir: PathBuf,
}

pub fn repo_modules(repo: &Path) -> Result<Vec<RepoModule>> {
    let mut out = Vec::new();
    let dir = repo.join("modules");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out);
    };
    for e in entries.flatten() {
        let d = e.path();
        let card_file = d.join("module.toml");
        if !card_file.is_file() {
            continue;
        }
        let card = Card::load(&card_file)?;
        let dirname = e.file_name().to_string_lossy().into_owned();
        let name = card.name.unwrap_or(dirname.clone());
        if name != dirname || !poste_core::modules::valid_name(&name) {
            bail!(
                "{} : le nom « {name} » doit être valide et égal au nom du dossier « {dirname} »",
                card_file.display()
            );
        }
        out.push(RepoModule { name, dir: d });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Tous les fichiers (récursivement) sous `dir`, triés.
fn files_under(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("lecture de {}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files_under(&p, out)?;
        } else if p.is_file() {
            out.push(p);
        }
    }
    Ok(())
}

/// La liste de tout ce que le dépôt veut poser.
pub fn desired(repo: &Path, env: &Env) -> Result<Vec<Item>> {
    let release = repo.join("target/release");
    let mut items = vec![Item {
        module: "poste".into(),
        src: release.join("poste"),
        dest: env.bin_dir().join("poste"),
    }];
    for m in repo_modules(repo)? {
        let base = env.modules_dir().join(&m.name);
        let prog = format!("poste-{}", m.name);
        // Module en Rust (Cargo.toml) : binaire construit. Module-script (python, shell…) :
        // le programme `poste-<nom>` du dossier du module, copié tel quel.
        let src = if m.dir.join("Cargo.toml").is_file() {
            release.join(&prog)
        } else {
            m.dir.join(&prog)
        };
        items.push(Item {
            module: m.name.clone(),
            src,
            dest: base.join(&prog),
        });
        items.push(Item {
            module: m.name.clone(),
            src: m.dir.join("module.toml"),
            dest: base.join("module.toml"),
        });
        // Dossiers facultatifs copiés tels quels (`bin/` : programmes annexes du module).
        for sub in ["island", "hypr", "bin"] {
            let root = m.dir.join(sub);
            if root.is_dir() {
                let mut files = Vec::new();
                files_under(&root, &mut files)?;
                for f in files {
                    let rel = f.strip_prefix(&m.dir).expect("sous le dossier du module");
                    items.push(Item {
                        module: m.name.clone(),
                        src: f.clone(),
                        dest: base.join(rel),
                    });
                }
            }
        }
    }
    Ok(items)
}

pub fn classify(item: &Item, manifest: &Manifest) -> Result<Action> {
    if std::fs::symlink_metadata(&item.dest).is_err() {
        return Ok(Action::Create);
    }
    if item.dest.is_dir() {
        return Ok(Action::Conflict);
    }
    let src = sha256_file(&item.src)?;
    let dest = sha256_file(&item.dest)?;
    let entry = manifest.get(&item.dest);
    Ok(if src == dest {
        match entry {
            Some(e) if e.sha256 == src => Action::Unchanged,
            _ => Action::Adopt,
        }
    } else {
        match entry {
            Some(e) => Action::Update {
                by_hand: e.sha256 != dest,
            },
            None => Action::Conflict,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Entry;
    use std::fs;

    fn item(d: &Path, src: &str, dest: &str) -> Item {
        let (s, t) = (d.join("src"), d.join("dest"));
        fs::write(&s, src).unwrap();
        if !dest.is_empty() {
            fs::write(&t, dest).unwrap();
        }
        Item {
            module: "m".into(),
            src: s,
            dest: t,
        }
    }

    fn entry_for(it: &Item, content: &str) -> Manifest {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("c");
        fs::write(&f, content).unwrap();
        let mut m = Manifest::default();
        m.upsert(Entry {
            path: it.dest.to_string_lossy().into(),
            sha256: sha256_file(&f).unwrap(),
            module: "m".into(),
            installed_at: "t".into(),
        });
        m
    }

    #[test]
    fn classement() {
        let d = tempfile::tempdir().unwrap();
        let none = Manifest::default();

        let it = item(d.path(), "a", "");
        assert_eq!(classify(&it, &none).unwrap(), Action::Create);

        let it = item(d.path(), "a", "a");
        assert_eq!(classify(&it, &none).unwrap(), Action::Adopt);
        assert_eq!(
            classify(&it, &entry_for(&it, "a")).unwrap(),
            Action::Unchanged
        );

        // Différent et inconnu : on n'y touche pas.
        let it = item(d.path(), "a", "autre");
        assert_eq!(classify(&it, &none).unwrap(), Action::Conflict);
        // Différent, posé par nous et intact : mise à jour.
        assert_eq!(
            classify(&it, &entry_for(&it, "autre")).unwrap(),
            Action::Update { by_hand: false }
        );
        // Différent, posé par nous puis modifié à la main.
        assert_eq!(
            classify(&it, &entry_for(&it, "ancien")).unwrap(),
            Action::Update { by_hand: true }
        );
    }

    #[test]
    fn liste_du_depot_avec_dossiers_facultatifs() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        let m = repo.join("modules/foo");
        fs::create_dir_all(m.join("island/sub")).unwrap();
        fs::create_dir_all(m.join("hypr")).unwrap();
        fs::write(m.join("module.toml"), "name = \"foo\"\n").unwrap();
        fs::write(m.join("island/sub/x.qml"), "").unwrap();
        fs::write(m.join("hypr/foo.conf"), "").unwrap();
        // Dossier sans module.toml : ignoré.
        fs::create_dir_all(repo.join("modules/bar")).unwrap();

        let env = Env::with_home(d.path().join("home"));
        let items = desired(&repo, &env).unwrap();
        let dests: Vec<String> = items.iter().map(|i| env.display(&i.dest)).collect();
        assert!(dests.contains(&"~/.local/bin/poste".to_string()));
        assert!(dests.contains(&"~/.local/share/poste/modules/foo/poste-foo".to_string()));
        assert!(dests.contains(&"~/.local/share/poste/modules/foo/module.toml".to_string()));
        assert!(dests.contains(&"~/.local/share/poste/modules/foo/island/sub/x.qml".to_string()));
        assert!(dests.contains(&"~/.local/share/poste/modules/foo/hypr/foo.conf".to_string()));
        assert_eq!(items.len(), 5);
    }

    #[test]
    fn nom_different_du_dossier_refuse() {
        let d = tempfile::tempdir().unwrap();
        let m = d.path().join("modules/foo");
        fs::create_dir_all(&m).unwrap();
        fs::write(m.join("module.toml"), "name = \"autre\"\n").unwrap();
        assert!(repo_modules(d.path()).is_err());
    }
}
