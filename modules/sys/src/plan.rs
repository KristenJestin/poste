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
    /// Program the module needs (`requires` in `module.toml`).
    pub requires: Option<String>,
    pub description: String,
}

impl RepoModule {
    /// The required program, when it cannot be found on this machine.
    pub fn missing(&self, env: &Env) -> Option<&str> {
        self.requires
            .as_deref()
            .filter(|r| !crate::provision::have(env, r))
    }
}

pub fn repo_modules(repo: &Path) -> Result<Vec<RepoModule>> {
    read_modules(&repo.join("modules"))
}

/// The add-ons of the repository (`addons/<name>/`): same layout as a module, each plugging an
/// outside program into the desktop. Installed only on a machine that lists it in `addons`.
pub fn repo_addons(repo: &Path) -> Result<Vec<RepoModule>> {
    let addons = read_modules(&repo.join("addons"))?;
    let modules = repo_modules(repo)?;
    if let Some(a) = addons
        .iter()
        .find(|a| modules.iter().any(|m| m.name == a.name))
    {
        bail!("add-on « {} » has the name of a module", a.name);
    }
    Ok(addons)
}

fn read_modules(dir: &Path) -> Result<Vec<RepoModule>> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
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
        out.push(RepoModule {
            name,
            dir: d,
            requires: card.requires,
            description: card.description,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// What must be installed on this machine: every module of the repository except those in
/// `disabled` (`sys` can never be disabled), plus the add-ons listed in `addons`. A module or
/// add-on whose required program is missing is left out.
pub fn enabled_modules(repo: &Path, env: &Env) -> Result<Vec<RepoModule>> {
    let cfg = poste_core::Config::load(env)?;
    let modules = repo_modules(repo)?
        .into_iter()
        .filter(|m| m.name == "sys" || !cfg.is_disabled(&m.name));
    let addons = repo_addons(repo)?
        .into_iter()
        .filter(|a| cfg.addons.contains(&a.name));
    let mut out: Vec<RepoModule> = modules
        .chain(addons)
        .filter(|m| m.missing(env).is_none())
        .collect();
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
    for m in enabled_modules(repo, env)? {
        let base = env.modules_dir().join(&m.name);
        let prog = format!("poste-{}", m.name);
        // Module en Rust (Cargo.toml) : binaire construit. Module-script (python, shell…) :
        // le programme `poste-<nom>` du dossier du module, copié tel quel. Une version
        // installée n'a plus de Cargo.toml : un module sans script y est un binaire construit.
        let script = m.dir.join(&prog);
        let src = if m.dir.join("Cargo.toml").is_file() || !script.is_file() {
            release.join(&prog)
        } else {
            script
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
        for sub in ["island", "hypr", "bin", "systemd"] {
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
    fn binaire_ou_script_selon_le_module() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        // Module en Rust dans le clone, module-script, et module en Rust d'une version
        // installée (l'archive n'a pas son Cargo.toml).
        for (name, file) in [
            ("crate", Some("Cargo.toml")),
            ("script", Some("poste-script")),
            ("installed", None),
        ] {
            let m = repo.join("modules").join(name);
            fs::create_dir_all(&m).unwrap();
            fs::write(m.join("module.toml"), format!("name = \"{name}\"\n")).unwrap();
            if let Some(f) = file {
                fs::write(m.join(f), "").unwrap();
            }
        }
        let env = Env::with_home(d.path().join("home"));
        let items = desired(&repo, &env).unwrap();
        let src = |prog: &str| {
            items
                .iter()
                .find(|i| i.dest.ends_with(prog))
                .map(|i| i.src.strip_prefix(&repo).unwrap().to_path_buf())
                .unwrap()
        };
        assert_eq!(src("poste-crate"), Path::new("target/release/poste-crate"));
        assert_eq!(
            src("poste-script"),
            Path::new("modules/script/poste-script")
        );
        assert_eq!(
            src("poste-installed"),
            Path::new("target/release/poste-installed")
        );
    }

    #[test]
    fn module_left_out_without_its_program() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        for (name, requires) in [
            ("present", "poste-test-present-player"),
            ("absent", "poste-test-absent-player"),
        ] {
            let m = repo.join("modules").join(name);
            fs::create_dir_all(&m).unwrap();
            fs::write(
                m.join("module.toml"),
                format!("name = \"{name}\"\nrequires = \"{requires}\"\n"),
            )
            .unwrap();
            fs::write(m.join(format!("poste-{name}")), "").unwrap();
        }
        let bin = d.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join("poste-test-present-player"), "").unwrap();
        let mut env = Env::with_home(d.path().join("home"));
        env.path = vec![bin];

        let names: Vec<String> = enabled_modules(&repo, &env)
            .unwrap()
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["present"]);
        let all = repo_modules(&repo).unwrap();
        let absent = all.iter().find(|m| m.name == "absent").unwrap();
        assert_eq!(absent.missing(&env), Some("poste-test-absent-player"));
    }

    #[test]
    fn addons_only_when_chosen() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        for (kind, name) in [
            ("modules", "base"),
            ("addons", "chosen"),
            ("addons", "other"),
        ] {
            let m = repo.join(kind).join(name);
            fs::create_dir_all(&m).unwrap();
            fs::write(m.join("module.toml"), format!("name = \"{name}\"\n")).unwrap();
            fs::write(m.join(format!("poste-{name}")), "").unwrap();
        }
        let env = Env::with_home(d.path().join("home"));
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "addons = [\"chosen\"]\n").unwrap();

        let names: Vec<String> = enabled_modules(&repo, &env)
            .unwrap()
            .into_iter()
            .map(|m| m.name)
            .collect();
        assert_eq!(names, ["base", "chosen"]);
        let dests: Vec<String> = desired(&repo, &env)
            .unwrap()
            .iter()
            .map(|i| env.display(&i.dest))
            .collect();
        assert!(dests.contains(&"~/.local/share/poste/modules/chosen/poste-chosen".to_string()));

        // An add-on may not take the name of a module.
        let m = repo.join("addons/base");
        fs::create_dir_all(&m).unwrap();
        fs::write(m.join("module.toml"), "name = \"base\"\n").unwrap();
        assert!(repo_addons(&repo).is_err());
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
