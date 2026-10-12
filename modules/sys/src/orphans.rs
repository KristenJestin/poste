//! Orphelins : ce que poste a posé et dont la source n'existe plus.
//!
//! - un lien de `~` qui pointe vers `<dépôt>/config/…` et dont la cible a disparu (fichier
//!   renommé ou retiré du dépôt) ;
//! - un fichier de module (`~/.local/share/poste/modules/…`) noté dans `installed.json`, que le
//!   dépôt ne produit plus (module supprimé, fichier renommé) et qui est resté tel qu'installé ;
//!   s'il a déjà disparu de `~`, il n'y a rien à retirer : son entrée sort du manifeste.
//!
//! Jamais un fichier ordinaire qui n'est pas dans `installed.json`, jamais un fichier de module
//! modifié à la main. Le retrait passe par un aperçu et une confirmation.

use crate::config::{self, Layout};
use crate::manifest::Manifest;
use crate::plan::Item;
use crate::util::sha256_file;
use anyhow::Result;
use poste_core::Env;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Lien de config dont la source du dépôt a disparu.
    Link { target: PathBuf },
    /// Fichier de module que le dépôt ne produit plus.
    ModuleFile { module: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Orphan {
    pub path: PathBuf,
    pub kind: Kind,
}

/// Cible réelle (chemin absolu) d'un lien, sans exiger qu'elle existe.
fn link_target(link: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_link(link).ok()?;
    Some(if raw.is_absolute() {
        raw
    } else {
        link.parent()?.join(raw)
    })
}

/// Un lien de config orphelin : il pointe sous `<dépôt>/config/` et rien n'existe là.
fn dangling_config_link(link: &Path, config_root: &Path) -> Option<PathBuf> {
    let md = std::fs::symlink_metadata(link).ok()?;
    if !md.file_type().is_symlink() {
        return None;
    }
    let target = link_target(link)?;
    (target.starts_with(config_root) && std::fs::symlink_metadata(&target).is_err())
        .then_some(target)
}

pub fn find(
    repo: &Path,
    env: &Env,
    layout: Option<&Layout>,
    manifest: &Manifest,
    items: &[Item],
) -> Result<Vec<Orphan>> {
    let mut roots = vec![repo.join("config")];
    if let Some(d) = layout.and_then(|l| l.data_root.clone()) {
        roots.push(d);
    }
    let mut out: Vec<Orphan> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let wanted: HashSet<PathBuf> = layout
        .map(|l| {
            l.entries
                .iter()
                .chain(l.local.iter())
                .map(|e| e.dest.clone())
                .collect()
        })
        .unwrap_or_default();

    // Liens de config : chemins notés dans le manifeste + enfants des dossiers gérés.
    let mut candidates: BTreeSet<PathBuf> = manifest
        .files
        .iter()
        .filter(|e| e.module == config::MODULE)
        .map(|e| PathBuf::from(&e.path))
        .collect();
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    if let Some(l) = layout {
        for e in &l.entries {
            if let Some(p) = e.dest.parent() {
                dirs.insert(p.to_path_buf());
            }
        }
    }
    for p in candidates.clone() {
        if let Some(parent) = p.parent() {
            dirs.insert(parent.to_path_buf());
        }
    }
    for d in dirs {
        // Un dossier réel de ~ seulement (pas l'intérieur d'un dossier lié vers le dépôt).
        let Ok(md) = std::fs::symlink_metadata(&d) else {
            continue;
        };
        if !md.is_dir() || !d.starts_with(&env.home) {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                candidates.insert(e.path());
            }
        }
    }
    for p in candidates {
        if wanted.contains(&p) || !seen.insert(p.clone()) {
            continue;
        }
        if let Some(target) = roots.iter().find_map(|r| dangling_config_link(&p, r)) {
            out.push(Orphan {
                path: p,
                kind: Kind::Link { target },
            });
        }
    }

    // Fichiers de module que le dépôt ne produit plus.
    let produced: HashSet<&Path> = items.iter().map(|i| i.dest.as_path()).collect();
    let modules_dir = env.modules_dir();
    for e in &manifest.files {
        let p = Path::new(&e.path);
        if e.module == config::MODULE || produced.contains(p) || !p.starts_with(&modules_dir) {
            continue;
        }
        let intact = matches!(sha256_file(p), Ok(h) if h == e.sha256);
        if intact || p.symlink_metadata().is_err() {
            out.push(Orphan {
                path: p.to_path_buf(),
                kind: Kind::ModuleFile {
                    module: e.module.clone(),
                },
            });
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

pub fn print(env: &Env, orphans: &[Orphan]) {
    if orphans.is_empty() {
        return;
    }
    let off = poste_core::Config::load(env)
        .map(|c| c.disabled)
        .unwrap_or_default();
    println!("\nOrphelins (à retirer, poste les avait posés) :");
    for o in orphans {
        match &o.kind {
            Kind::Link { target } => println!(
                "  lien orphelin    {}  -> {}  (source disparue du dépôt)",
                env.display(&o.path),
                target.display()
            ),
            Kind::ModuleFile { module } => println!(
                "  fichier orphelin {}  [{module}]  ({})",
                env.display(&o.path),
                if o.path.symlink_metadata().is_err() {
                    "déjà disparu, oublié du manifeste"
                } else if off.contains(module) {
                    "module désactivé sur cette machine"
                } else {
                    "le dépôt ne le produit plus"
                }
            ),
        }
    }
}

/// Retire les orphelins (revérifiés au dernier moment) et les oublie dans le manifeste.
/// Un fichier de module est d'abord sauvegardé ; ses dossiers devenus vides sont retirés.
pub fn apply(
    env: &Env,
    repo: &Path,
    orphans: &[Orphan],
    stamp: &str,
    manifest: &mut Manifest,
) -> Result<usize> {
    let config_root = repo.join("config");
    let mut n = 0;
    for o in orphans {
        match &o.kind {
            Kind::Link { .. } => {
                // Le lien doit toujours pointer vers une cible disparue du dépôt.
                if dangling_config_link(&o.path, &config_root).is_none() {
                    continue;
                }
                std::fs::remove_file(&o.path)?;
            }
            Kind::ModuleFile { .. } if o.path.symlink_metadata().is_err() => {}
            Kind::ModuleFile { .. } => {
                let still = manifest
                    .get(&o.path)
                    .is_some_and(|e| matches!(sha256_file(&o.path), Ok(h) if h == e.sha256));
                if !still {
                    continue;
                }
                let bak = crate::util::backup(env, &o.path, stamp)?;
                println!("  sauvegarde : {}", env.display(&bak));
                std::fs::remove_file(&o.path)?;
                let stop = env.modules_dir();
                for dir in o.path.ancestors().skip(1) {
                    if dir == stop || !dir.starts_with(&stop) || std::fs::remove_dir(dir).is_err() {
                        break;
                    }
                }
            }
        }
        let key = o.path.to_string_lossy();
        manifest.files.retain(|e| e.path != key);
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Entry;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn entry(p: &Path, sha: &str, module: &str) -> Entry {
        Entry {
            path: p.to_string_lossy().into(),
            sha256: sha.into(),
            module: module.into(),
            installed_at: "t".into(),
        }
    }

    #[test]
    fn lien_orphelin_apres_renommage_et_rien_d_autre() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        let home = d.path().join("home");
        fs::create_dir_all(repo.join("config/.config/a")).unwrap();
        fs::create_dir_all(home.join(".config/a")).unwrap();
        fs::write(repo.join("config/.config/a/nouveau"), "x").unwrap();
        let env = Env::with_home(&home);
        // Ancien lien (source renommée), nouveau lien, fichier ordinaire, lien étranger cassé.
        symlink(
            repo.join("config/.config/a/ancien"),
            home.join(".config/a/ancien"),
        )
        .unwrap();
        symlink(
            repo.join("config/.config/a/nouveau"),
            home.join(".config/a/nouveau"),
        )
        .unwrap();
        fs::write(home.join(".config/a/ordinaire"), "perso").unwrap();
        symlink("/nulle/part", home.join(".config/a/etranger")).unwrap();
        let layout = config::load(&repo, &env).unwrap().unwrap();
        let mut m = Manifest::default();
        m.files
            .push(entry(&home.join(".config/a/ancien"), "h", config::MODULE));
        let o = find(&repo, &env, Some(&layout), &m, &[]).unwrap();
        assert_eq!(o.len(), 1, "{o:?}");
        assert!(o[0].path.ends_with(".config/a/ancien"));
        assert_eq!(apply(&env, &repo, &o, "t", &mut m).unwrap(), 1);
        assert!(!home.join(".config/a/ancien").is_symlink());
        assert!(home.join(".config/a/nouveau").is_symlink());
        assert!(home.join(".config/a/ordinaire").is_file());
        assert!(home.join(".config/a/etranger").is_symlink());
        assert!(m.files.is_empty());
    }

    #[test]
    fn fichier_de_module_disparu_retire_seulement_s_il_est_intact() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let env = Env::with_home(d.path().join("home"));
        let dir = env.modules_dir().join("rec/hypr");
        fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.lua"), dir.join("b.lua"));
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();
        let mut m = Manifest::default();
        m.files.push(entry(&a, &sha256_file(&a).unwrap(), "rec"));
        m.files.push(entry(&b, "autre-empreinte", "rec")); // modifié à la main
        let o = find(&repo, &env, None, &m, &[]).unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].path, a);
        apply(&env, &repo, &o, "t", &mut m).unwrap();
        assert!(!a.exists() && b.exists());
        // Un module encore produit par le dépôt n'est pas orphelin.
        let item = Item {
            module: "rec".into(),
            src: a.clone(),
            dest: b.clone(),
        };
        m.files[0].sha256 = sha256_file(&b).unwrap();
        assert!(find(&repo, &env, None, &m, &[item]).unwrap().is_empty());
    }

    #[test]
    fn fichier_de_module_deja_disparu_oublie_du_manifeste() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let env = Env::with_home(d.path().join("home"));
        let gone = env.modules_dir().join("ancien/poste-ancien");
        let mut m = Manifest::default();
        m.files.push(entry(&gone, "empreinte", "ancien"));
        let o = find(&repo, &env, None, &m, &[]).unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(apply(&env, &repo, &o, "t", &mut m).unwrap(), 1);
        assert!(m.files.is_empty());
    }
}
