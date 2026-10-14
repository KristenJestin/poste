//! `poste sys sync` : met la machine à jour selon sa source.
//!
//! - dev : `git pull --ff-only` dans le clone, `current` sur le clone, puis `setup` ;
//! - release : dernière version publiée (téléchargée et vérifiée si besoin), `current` dessus,
//!   puis `setup`, puis élagage des anciennes versions.
//!
//! Dans les deux cas le dépôt de données (`data`) reste en git (`git pull --ff-only`). Ce qu'il
//! faut recharger vient des fichiers changés : `git diff` pour un dépôt git, comparaison des
//! deux racines pour un changement de version.

use crate::reload::{self, Need};
use crate::versions::{self, Fetcher};
use crate::{setup, util};
use anyhow::{bail, Context, Result};
use poste_core::{Config, Env, Source};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Ce que `run` a fait : code de sortie et ce qu'il faut recharger.
pub struct Synced {
    pub code: ExitCode,
    pub need: Need,
}

/// `git pull --ff-only` dans chaque dépôt ; renvoie le commit de chacun avant le pull.
fn pull(dirs: &[PathBuf], dry_run: bool) -> Result<Vec<(PathBuf, String)>> {
    let mut before = Vec::new();
    for dir in dirs {
        if dry_run {
            println!(
                "Simulation : git pull --ff-only non lancé ({}).",
                dir.display()
            );
            continue;
        }
        before.push((dir.clone(), reload::head(dir)?));
        let st = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["pull", "--ff-only"])
            .status()
            .context("impossible de lancer git")?;
        if !st.success() {
            bail!(
                "git pull --ff-only a échoué dans {}, setup non lancé",
                dir.display()
            );
        }
    }
    Ok(before)
}

/// Le dépôt de données de la machine, s'il est déclaré (doit être un dépôt git).
fn data_repo(env: &Env, cfg: &Config) -> Result<Option<PathBuf>> {
    let Some(d) = cfg.data_path(env) else {
        return Ok(None);
    };
    if !d.join(".git").exists() {
        bail!(
            "data = {} : pas un dépôt git (./setup --data <adresse> le clone)",
            d.display()
        );
    }
    Ok(Some(d))
}

pub fn run(env: &Env, cfg: &Config, f: &Fetcher, yes: bool, dry_run: bool) -> Result<Synced> {
    let opts = setup::Opts {
        yes,
        dry_run,
        ..Default::default()
    };
    let mut dirs = Vec::new();
    if cfg.source == Source::Dev {
        dirs.push(util::clone(env, cfg)?);
    }
    dirs.extend(data_repo(env, cfg)?);
    let old = versions::current_target(env);
    let before = pull(&dirs, dry_run)?;
    let mut need = Need::default();

    let root = match cfg.source {
        Source::Dev => versions::dev_current(env, cfg, dry_run)?,
        Source::Release => match versions::fetch(env, f, dry_run)? {
            Some(v) => {
                if dry_run {
                    v.dir
                } else {
                    if versions::current_target(env) != std::fs::canonicalize(&v.dir).ok() {
                        versions::point_current(env, &v.dir)?;
                    }
                    env.current_link()
                }
            }
            None => {
                // Simulation sans la nouvelle version : rien d'autre à montrer.
                return Ok(Synced {
                    code: ExitCode::SUCCESS,
                    need,
                });
            }
        },
    };
    // Changement de racine (nouvelle version, ou passage au clone) : fichiers comparés.
    if let Some(o) = &old {
        versions::add_root_changes(&mut need, Some(o), &root)?;
    }
    let done = setup::run(env, &root, &opts)?;
    if done.config_changed {
        need.shell = true;
        need.hypr = true;
    }
    for (dir, from) in &before {
        reload::add_changes(&mut need, dir, from, &reload::head(dir)?)?;
    }
    if cfg.source == Source::Release && !dry_run {
        let removed = versions::prune(env, versions::KEEP)?;
        if !removed.is_empty() {
            println!("Anciennes versions retirées : {}.", removed.join(", "));
        }
    }
    Ok(Synced {
        code: done.code,
        need,
    })
}

/// Pour les messages : la racine lue, en clair.
pub fn describe_root(env: &Env, root: &Path) -> String {
    match versions::version_of(env, root) {
        Some(v) => format!("version {} ({})", v.short(), v.date),
        None => format!(
            "clone {}",
            std::fs::canonicalize(root)
                .map(|p| env.display(&p))
                .unwrap_or_else(|_| env.display(root))
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;
    use crate::versions::tests::{fake_curl, make_root, publish};
    use std::fs;
    use std::os::unix::fs::symlink;

    /// HOME temporaire d'une machine existante : clone dans `repo`, liens posés par l'ancien
    /// `setup` (vers le clone, notés dans installed.json), un lien étranger et un fichier à soi.
    struct Machine {
        d: tempfile::TempDir,
        env: Env,
        repo: PathBuf,
    }

    fn machine() -> Machine {
        let d = tempfile::tempdir().unwrap();
        let env = Env::with_home(d.path().join("home"));
        let repo = d.path().join("repo");
        make_root(&repo, "", "", "{}");
        fs::write(repo.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(repo.join("config/.config/a/autre"), "o").unwrap();
        fs::write(repo.join("config/.config/a/perso"), "dépôt").unwrap();
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(
            env.config_file(),
            format!("repo = \"{}\"\n", repo.display()),
        )
        .unwrap();
        // Pas d'unité systemd dans ces racines : `systemctl` n'est jamais appelé.
        Machine { d, env, repo }
    }

    fn cfg(env: &Env) -> Config {
        Config::load(env).unwrap()
    }

    fn opts() -> setup::Opts {
        setup::Opts {
            yes: true,
            skip_build: true,
            ..Default::default()
        }
    }

    /// L'ancien `setup` : liens directs vers le clone, notés dans le manifeste.
    fn old_setup(m: &Machine) {
        crate::setup::run(&m.env, &m.repo, &opts()).unwrap();
        let l = m.env.home.join(".config/a/neuf");
        assert_eq!(
            fs::read_link(&l).unwrap(),
            m.repo.join("config/.config/a/neuf")
        );
    }

    #[test]
    fn migration_dev_ne_remplace_que_les_liens_poses_par_setup() {
        let m = machine();
        let home = m.env.home.clone();
        old_setup(&m);
        // Un lien vers le clone que setup n'a pas noté (posé à la main) : jamais touché.
        let autre = home.join(".config/a/autre");
        fs::remove_file(&autre).unwrap();
        symlink(m.repo.join("config/.config/a/autre"), &autre).unwrap();
        let mut man = Manifest::load(&m.env.manifest_file()).unwrap();
        man.files.retain(|e| !e.path.ends_with("a/autre"));
        man.save(&m.env.manifest_file()).unwrap();
        // Un fichier ordinaire différent : conflit, jamais écrasé.
        let perso = home.join(".config/a/perso");
        fs::remove_file(&perso).unwrap();
        fs::write(&perso, "maison").unwrap();

        // Aperçu : rien n'est écrit, ni current ni liens.
        let c = cfg(&m.env);
        let root = versions::ensure_current(&m.env, &c, true).unwrap();
        crate::setup::run(
            &m.env,
            &root,
            &setup::Opts {
                dry_run: true,
                ..opts()
            },
        )
        .unwrap();
        assert!(fs::symlink_metadata(m.env.current_link()).is_err());
        assert_eq!(
            fs::read_link(home.join(".config/a/neuf")).unwrap(),
            m.repo.join("config/.config/a/neuf")
        );

        // Pour de vrai : current -> clone, le lien noté repasse par current (ancien sauvegardé).
        let root = versions::ensure_current(&m.env, &c, false).unwrap();
        assert_eq!(root, m.env.current_link());
        crate::setup::run(&m.env, &root, &opts()).unwrap();
        let cur = m.env.current_link();
        assert_eq!(
            fs::read_link(home.join(".config/a/neuf")).unwrap(),
            cur.join("config/.config/a/neuf")
        );
        assert_eq!(
            fs::read_link(&autre).unwrap(),
            m.repo.join("config/.config/a/autre"),
            "lien inconnu du manifeste : intact"
        );
        assert_eq!(fs::read_to_string(&perso).unwrap(), "maison");
        let baks: Vec<String> = fs::read_dir(m.env.backup_dir())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(baks.iter().any(|b| b.contains("neuf")), "{baks:?}");
        assert!(!baks
            .iter()
            .any(|b| b.contains("autre") || b.contains("perso")));
        // Relancé : rien à faire.
        let before = fs::read_to_string(m.env.manifest_file()).unwrap();
        crate::setup::run(&m.env, &root, &opts()).unwrap();
        assert_eq!(fs::read_to_string(m.env.manifest_file()).unwrap(), before);
    }

    #[test]
    fn sync_release_installe_bascule_et_compare_les_racines() {
        let m = machine();
        let home = m.env.home.clone();
        old_setup(&m);
        // La machine passe en release : version publiée, téléchargée par le faux curl.
        let src = m.d.path().join("v1");
        make_root(&src, "1111111", "2026-10-01T10:00:00Z", "{}");
        let pubdir = m.d.path().join("pub");
        publish(&src, &pubdir);
        let (curl, _) = fake_curl(m.d.path(), &pubdir);
        let f = Fetcher::with(&curl, "https://example.com/r");
        crate::machine::set_key(&m.env.config_file(), "source", "release").unwrap();

        // Simulation : rien n'est installé ni basculé.
        let s = run(&m.env, &cfg(&m.env), &f, true, true).unwrap();
        assert_eq!(format!("{:?}", s.code), format!("{:?}", ExitCode::SUCCESS));
        assert!(versions::installed(&m.env).is_empty());

        let s = run(&m.env, &cfg(&m.env), &f, true, false).unwrap();
        assert_eq!(format!("{:?}", s.code), format!("{:?}", ExitCode::SUCCESS));
        let v1 = m.env.versions_dir().join("1111111");
        assert_eq!(versions::current_target(&m.env), fs::canonicalize(&v1).ok());
        // Liens migrés vers current (qui désigne la version), binaire de la version posé.
        let link = home.join(".config/a/neuf");
        assert_eq!(
            fs::read_link(&link).unwrap(),
            m.env.current_link().join("config/.config/a/neuf")
        );
        assert_eq!(
            fs::read_to_string(m.env.bin_dir().join("poste")).unwrap(),
            "bin 1111111"
        );

        // Nouvelle version dont seule l'île d'un module change : île à recharger, pas Hyprland.
        let src2 = m.d.path().join("v2");
        make_root(&src2, "2222222", "2026-10-02T10:00:00Z", "{\"x\":1}");
        publish(&src2, &pubdir);
        let s = run(&m.env, &cfg(&m.env), &f, true, false).unwrap();
        assert!(s.need.shell && !s.need.hypr);
        assert_eq!(
            fs::read_to_string(m.env.modules_dir().join("rec/island/launcher.json")).unwrap(),
            "{\"x\":1}"
        );
        // Rien de neuf : rien à recharger, aucun téléchargement de l'archive.
        let s = run(&m.env, &cfg(&m.env), &f, true, false).unwrap();
        assert!(!s.need.any());
        assert_eq!(versions::installed(&m.env).len(), 2);

        // Retour en dev : current sur le clone, liens inchangés (ils passent par current).
        let sw = versions::use_target(&m.env, &cfg(&m.env), &versions::Target::Dev, &f, &opts())
            .unwrap();
        assert!(
            sw.need.shell,
            "l'île du clone diffère de celle de la version"
        );
        assert_eq!(
            versions::current_target(&m.env),
            fs::canonicalize(&m.repo).ok()
        );
        assert_eq!(cfg(&m.env).source, Source::Dev);
        assert_eq!(
            fs::read_link(&link).unwrap(),
            m.env.current_link().join("config/.config/a/neuf")
        );
        // Retour arrière sur la première version, par le début de son commit.
        versions::use_target(
            &m.env,
            &cfg(&m.env),
            &versions::Target::parse("111"),
            &f,
            &opts(),
        )
        .unwrap();
        assert_eq!(versions::current_target(&m.env), fs::canonicalize(&v1).ok());
        assert_eq!(cfg(&m.env).source, Source::Release);
        // Écrire dans le dépôt est refusé depuis une version installée.
        assert!(versions::ensure_writable(&m.env, &m.env.current_link(), "add").is_err());
    }
}
