//! Orphans: what poste put in place and whose source no longer exists.
//!
//! - a link of `~` that points to `<repository>/config/…` and whose target is gone (file
//!   renamed or removed from the repository); all the poste roots count: `current`, the clone,
//!   the installed versions, the data repository;
//! - a module file (`~/.local/share/poste/modules/…`) noted in `installed.json`, that the
//!   repository no longer produces (module deleted, file renamed) and that was left as installed;
//!   if it has already disappeared from `~`, there is nothing to remove: its entry leaves the manifest.
//!
//! Never an ordinary file that is not in `installed.json`, never a module file
//! edited by hand. Removal goes through a preview and a confirmation.

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
    /// Config link whose source in the repository is gone.
    Link { target: PathBuf },
    /// Module file that the repository no longer produces.
    ModuleFile { module: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Orphan {
    pub path: PathBuf,
    pub kind: Kind,
}

/// Real target (absolute path) of a link, without requiring it to exist.
fn link_target(link: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_link(link).ok()?;
    Some(if raw.is_absolute() {
        raw
    } else {
        link.parent()?.join(raw)
    })
}

/// An orphan config link: it points under `<repository>/config/` and nothing exists there.
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
    // Broken `current` (version removed, link to redo): its links are to be redone, not
    // orphans. Likewise, only the versions still installed count as roots.
    if env.current_link().exists() {
        roots.push(env.current_link().join("config"));
    }
    if let Ok(rd) = std::fs::read_dir(env.versions_dir()) {
        roots.extend(
            rd.flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| e.path().join("config"))
                .filter(|p| p.is_dir()),
        );
    }
    if let Some(c) = poste_core::Config::load(env)?.repo_path(env) {
        roots.push(c.join("config"));
    }
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

    // Config links: paths noted in the manifest + children of the managed folders.
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
        // A real folder of ~ only (not the inside of a folder linked to the repository).
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

    // Module files that the repository no longer produces.
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
    println!("\nOrphans (to remove, poste had put them in place):");
    for o in orphans {
        match &o.kind {
            Kind::Link { target } => println!(
                "  orphan link {}  -> {}  (source gone from the repository)",
                env.display(&o.path),
                target.display()
            ),
            Kind::ModuleFile { module } => println!(
                "  orphan file {}  [{module}]  ({})",
                env.display(&o.path),
                if o.path.symlink_metadata().is_err() {
                    "already gone, forgotten from the manifest"
                } else if off.contains(module) {
                    "module disabled on this machine"
                } else {
                    "no longer installed here: gone from the repository, add-on not chosen, or its program missing"
                }
            ),
        }
    }
}

/// Removes the orphans (checked again at the last moment) and forgets them in the manifest.
/// A module file is backed up first; its folders that became empty are removed.
pub fn apply(
    env: &Env,
    _repo: &Path,
    orphans: &[Orphan],
    stamp: &str,
    manifest: &mut Manifest,
) -> Result<usize> {
    let mut n = 0;
    for o in orphans {
        match &o.kind {
            Kind::Link { target } => {
                // The link must still point to the same target, still gone.
                let still = std::fs::symlink_metadata(&o.path)
                    .is_ok_and(|m| m.file_type().is_symlink())
                    && link_target(&o.path).as_ref() == Some(target)
                    && std::fs::symlink_metadata(target).is_err();
                if !still {
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
    fn orphan_link_after_rename_and_nothing_else() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        let home = d.path().join("home");
        fs::create_dir_all(repo.join("config/.config/a")).unwrap();
        fs::create_dir_all(home.join(".config/a")).unwrap();
        fs::write(repo.join("config/.config/a/fresh"), "x").unwrap();
        let env = Env::with_home(&home);
        // Old link (renamed source), new link, ordinary file, broken foreign link.
        symlink(
            repo.join("config/.config/a/old"),
            home.join(".config/a/old"),
        )
        .unwrap();
        symlink(
            repo.join("config/.config/a/fresh"),
            home.join(".config/a/fresh"),
        )
        .unwrap();
        fs::write(home.join(".config/a/ordinaire"), "perso").unwrap();
        symlink("/nulle/part", home.join(".config/a/etranger")).unwrap();
        let layout = config::load(&repo, &env).unwrap().unwrap();
        let mut m = Manifest::default();
        m.files
            .push(entry(&home.join(".config/a/old"), "h", config::MODULE));
        let o = find(&repo, &env, Some(&layout), &m, &[]).unwrap();
        assert_eq!(o.len(), 1, "{o:?}");
        assert!(o[0].path.ends_with(".config/a/old"));
        assert_eq!(apply(&env, &repo, &o, "t", &mut m).unwrap(), 1);
        assert!(!home.join(".config/a/old").is_symlink());
        assert!(home.join(".config/a/fresh").is_symlink());
        assert!(home.join(".config/a/ordinaire").is_file());
        assert!(home.join(".config/a/etranger").is_symlink());
        assert!(m.files.is_empty());
    }

    #[test]
    fn broken_current_does_not_make_links_orphans() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        let home = d.path().join("home");
        fs::create_dir_all(repo.join("config/.config/a")).unwrap();
        fs::create_dir_all(home.join(".config/a")).unwrap();
        fs::write(repo.join("config/.config/a/x"), "x").unwrap();
        let env = Env::with_home(&home);
        // current points to a removed version; one link goes through current, another through
        // a version that is no longer installed, a third through an installed version.
        fs::create_dir_all(env.current_link().parent().unwrap()).unwrap();
        symlink(env.versions_dir().join("retiree"), env.current_link()).unwrap();
        let v = env.versions_dir().join("presente/config/.config/a");
        fs::create_dir_all(&v).unwrap();
        let links = [
            ("par_current", env.current_link().join("config/.config/a/x")),
            (
                "version_retiree",
                env.versions_dir().join("retiree/config/.config/a/x"),
            ),
            ("version_presente", v.join("disparu")),
        ];
        let mut m = Manifest::default();
        for (name, target) in &links {
            symlink(target, home.join(".config/a").join(name)).unwrap();
            m.files.push(entry(
                &home.join(".config/a").join(name),
                "h",
                config::MODULE,
            ));
        }
        let layout = config::load(&repo, &env).unwrap().unwrap();
        let o = find(&repo, &env, Some(&layout), &m, &[]).unwrap();
        // Only the link to a file gone from a version that is really there is an orphan.
        assert_eq!(o.len(), 1, "{o:?}");
        assert!(o[0].path.ends_with("version_presente"));
    }

    #[test]
    fn vanished_module_file_removed_only_if_intact() {
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
        m.files.push(entry(&b, "other-fingerprint", "rec")); // edited by hand
        let o = find(&repo, &env, None, &m, &[]).unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].path, a);
        apply(&env, &repo, &o, "t", &mut m).unwrap();
        assert!(!a.exists() && b.exists());
        // A module still produced by the repository is not an orphan.
        let item = Item {
            module: "rec".into(),
            src: a.clone(),
            dest: b.clone(),
        };
        m.files[0].sha256 = sha256_file(&b).unwrap();
        assert!(find(&repo, &env, None, &m, &[item]).unwrap().is_empty());
    }

    #[test]
    fn module_file_already_gone_forgotten_from_manifest() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        let env = Env::with_home(d.path().join("home"));
        let gone = env.modules_dir().join("old/poste-old");
        let mut m = Manifest::default();
        m.files.push(entry(&gone, "fingerprint", "old"));
        let o = find(&repo, &env, None, &m, &[]).unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(apply(&env, &repo, &o, "t", &mut m).unwrap(), 1);
        assert!(m.files.is_empty());
    }
}
