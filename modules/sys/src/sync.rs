//! `poste sys sync`: updates the machine according to its source.
//!
//! - dev: `git pull --ff-only` in the clone, `current` on the clone, then `setup`;
//! - release: latest published version (downloaded and verified if needed), `current` on it,
//!   then `setup`, then pruning of the old versions.
//!
//! In both cases the data repository (`data`) stays in git (`git pull --ff-only`). What
//! must be reloaded comes from the changed files: `git diff` for a git repository, comparison of the
//! two roots for a version change.

use crate::reload::{self, Need};
use crate::versions::{self, Fetcher};
use crate::{handoff, setup, util};
use anyhow::{bail, Context, Result};
use poste_core::{Config, Env, Source};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// What `run` did: exit code and what must be reloaded.
pub use versions::Switched as Synced;

/// `git pull --ff-only` in each repository; returns the commit of each one before the pull.
fn pull(dirs: &[PathBuf], dry_run: bool) -> Result<Vec<(PathBuf, String)>> {
    let mut before = Vec::new();
    for dir in dirs {
        if dry_run {
            println!("Dry run: git pull --ff-only not run ({}).", dir.display());
            continue;
        }
        before.push((dir.clone(), reload::head(dir)?));
        let st = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["pull", "--ff-only"])
            .status()
            .context("cannot run git")?;
        if !st.success() {
            bail!(
                "git pull --ff-only failed in {}, setup not run",
                dir.display()
            );
        }
    }
    Ok(before)
}

/// The machine's data repository, if declared (must be a git repository).
fn data_repo(env: &Env, cfg: &Config) -> Result<Option<PathBuf>> {
    let Some(d) = cfg.data_path(env) else {
        return Ok(None);
    };
    if !d.join(".git").exists() {
        bail!(
            "data = {}: not a git repository (./setup --data <address> clones it)",
            d.display()
        );
    }
    Ok(Some(d))
}

pub fn run(env: &Env, cfg: &Config, f: &Fetcher, yes: bool, dry_run: bool) -> Result<Synced> {
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
                // Simulation without the new version: nothing else to show.
                return Ok(Synced {
                    code: ExitCode::SUCCESS,
                    need,
                });
            }
        },
    };
    // Root change (new version, or switch to the clone): files compared.
    if let Some(o) = &old {
        versions::add_root_changes(&mut need, Some(o), &root)?;
    }
    for (dir, from) in &before {
        reload::add_changes(&mut need, dir, from, &reload::head(dir)?)?;
    }
    let prune = cfg.source == Source::Release;
    if dry_run {
        let opts = setup::Opts {
            yes,
            dry_run,
            link_root: versions::link_root(env, &root),
            ..Default::default()
        };
        return versions::finish(env, &root, need, &opts, false);
    }
    handoff::complete(env, old.as_deref(), need, yes, prune, false)
}

/// For messages: the root read, in plain words.
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

    /// Temporary HOME of an existing machine: clone in `repo`, links put in place by the old
    /// `setup` (to the clone, noted in installed.json), a foreign link and a file of its own.
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
        fs::write(repo.join("config/.config/a/other"), "o").unwrap();
        fs::write(repo.join("config/.config/a/mine"), "repository").unwrap();
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(
            env.config_file(),
            format!("repo = \"{}\"\n", repo.display()),
        )
        .unwrap();
        // No systemd unit in these roots: `systemctl` is never called.
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

    /// The old `setup`: direct links to the clone, noted in the manifest.
    fn old_setup(m: &Machine) {
        crate::setup::run(&m.env, &m.repo, &opts()).unwrap();
        let l = m.env.home.join(".config/a/neuf");
        assert_eq!(
            fs::read_link(&l).unwrap(),
            m.repo.join("config/.config/a/neuf")
        );
    }

    #[test]
    fn dev_migration_only_replaces_links_put_by_setup() {
        let m = machine();
        let home = m.env.home.clone();
        old_setup(&m);
        // A link to the clone that setup did not note (put in place by hand): never touched.
        let other = home.join(".config/a/other");
        fs::remove_file(&other).unwrap();
        symlink(m.repo.join("config/.config/a/other"), &other).unwrap();
        let mut man = Manifest::load(&m.env.manifest_file()).unwrap();
        man.files.retain(|e| !e.path.ends_with("a/other"));
        man.save(&m.env.manifest_file()).unwrap();
        // A different ordinary file: conflict, never overwritten.
        let mine = home.join(".config/a/mine");
        fs::remove_file(&mine).unwrap();
        fs::write(&mine, "maison").unwrap();

        // Preview: the plan shows the real migration (noted link put back to current, link put in place
        // by hand left), and nothing is written, neither current nor links nor manifest.
        let c = cfg(&m.env);
        let manifest_before = fs::read_to_string(m.env.manifest_file()).unwrap();
        let root = versions::ensure_current(&m.env, &c, true).unwrap();
        let link_root = versions::link_root(&m.env, &root);
        assert_eq!(link_root, Some(m.env.current_link()));
        let mut layout = crate::config::load(&root, &m.env).unwrap().unwrap();
        layout.relink(&m.env.current_link());
        let plan = crate::config::plan(&layout, &m.env, &Default::default()).unwrap();
        let op = |rel: &str| {
            plan.steps
                .iter()
                .find(|s| s.entry.rel == rel)
                .unwrap()
                .op
                .clone()
        };
        assert_eq!(op(".config/a/neuf"), crate::config::Op::Replace);
        assert_eq!(op(".config/a/other"), crate::config::Op::Done);
        assert_eq!(op(".config/a/mine"), crate::config::Op::Conflict);
        crate::setup::run(
            &m.env,
            &root,
            &setup::Opts {
                dry_run: true,
                link_root,
                ..opts()
            },
        )
        .unwrap();
        assert!(fs::symlink_metadata(m.env.current_link()).is_err());
        assert_eq!(
            fs::read_link(home.join(".config/a/neuf")).unwrap(),
            m.repo.join("config/.config/a/neuf")
        );
        assert_eq!(
            fs::read_to_string(m.env.manifest_file()).unwrap(),
            manifest_before
        );
        assert!(!m.env.backup_dir().exists());

        // For real: current -> clone, the noted link goes through current again (old one backed up).
        let root = versions::ensure_current(&m.env, &c, false).unwrap();
        assert_eq!(root, m.env.current_link());
        crate::setup::run(&m.env, &root, &opts()).unwrap();
        let cur = m.env.current_link();
        assert_eq!(
            fs::read_link(home.join(".config/a/neuf")).unwrap(),
            cur.join("config/.config/a/neuf")
        );
        assert_eq!(
            fs::read_link(&other).unwrap(),
            m.repo.join("config/.config/a/other"),
            "link unknown to the manifest: intact"
        );
        assert_eq!(fs::read_to_string(&mine).unwrap(), "maison");
        let baks: Vec<String> = fs::read_dir(m.env.backup_dir())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(baks.iter().any(|b| b.contains("neuf")), "{baks:?}");
        assert!(!baks
            .iter()
            .any(|b| b.contains("other") || b.contains("mine")));
        // Run again: nothing to do.
        let before = fs::read_to_string(m.env.manifest_file()).unwrap();
        crate::setup::run(&m.env, &root, &opts()).unwrap();
        assert_eq!(fs::read_to_string(m.env.manifest_file()).unwrap(), before);
    }

    #[test]
    fn sync_release_installs_switches_and_compares_roots() {
        let m = machine();
        let home = m.env.home.clone();
        old_setup(&m);
        // The machine switches to release: published version, downloaded by the fake curl.
        let src = m.d.path().join("v1");
        make_root(&src, "1111111", "2026-10-01T10:00:00Z", "{}");
        let pubdir = m.d.path().join("pub");
        publish(&src, &pubdir);
        let (curl, _) = fake_curl(m.d.path(), &pubdir);
        let f = Fetcher::with(&curl, "https://example.com/r");
        crate::machine::set_key(&m.env.config_file(), "source", "release").unwrap();

        // Dry run: nothing is installed or switched.
        let s = run(&m.env, &cfg(&m.env), &f, true, true).unwrap();
        assert_eq!(format!("{:?}", s.code), format!("{:?}", ExitCode::SUCCESS));
        assert!(versions::installed(&m.env).is_empty());

        let s = run(&m.env, &cfg(&m.env), &f, true, false).unwrap();
        assert_eq!(format!("{:?}", s.code), format!("{:?}", ExitCode::SUCCESS));
        let v1 = m.env.versions_dir().join("1111111");
        assert_eq!(versions::current_target(&m.env), fs::canonicalize(&v1).ok());
        // Links migrated to current (which points to the version), binary of the version put in place.
        let link = home.join(".config/a/neuf");
        assert_eq!(
            fs::read_link(&link).unwrap(),
            m.env.current_link().join("config/.config/a/neuf")
        );
        assert_eq!(
            fs::read_to_string(m.env.bin_dir().join("poste")).unwrap(),
            "bin 1111111"
        );

        // Preview of a switch to release: the links already go through current, nothing to re-link
        // even if the root read is another version than the one current points to.
        let mut layout = crate::config::load(&v1, &m.env).unwrap().unwrap();
        layout.relink(&m.env.current_link());
        let plan = crate::config::plan(&layout, &m.env, &Default::default()).unwrap();
        assert!(plan
            .steps
            .iter()
            .filter(|s| s.entry.rel == ".config/a/neuf")
            .all(|s| s.op == crate::config::Op::Done));

        // New version where only a module's island changes: island to reload, not Hyprland.
        let src2 = m.d.path().join("v2");
        make_root(&src2, "2222222", "2026-10-02T10:00:00Z", "{\"x\":1}");
        publish(&src2, &pubdir);
        let s = run(&m.env, &cfg(&m.env), &f, true, false).unwrap();
        assert!(s.need.shell && !s.need.hypr);
        assert_eq!(
            fs::read_to_string(m.env.modules_dir().join("rec/island/launcher.json")).unwrap(),
            "{\"x\":1}"
        );
        // Nothing new: nothing to reload, no download of the archive.
        let s = run(&m.env, &cfg(&m.env), &f, true, false).unwrap();
        assert!(!s.need.any());
        assert_eq!(versions::installed(&m.env).len(), 2);

        // Back to dev: current on the clone, links unchanged (they go through current).
        let sw = versions::use_target(&m.env, &cfg(&m.env), &versions::Target::Dev, &f, &opts())
            .unwrap();
        assert!(
            sw.need.shell,
            "the island of the clone differs from the one of the version"
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
        // Rollback to the first version, by the start of its commit.
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
        // Writing into the repository is refused from an installed version.
        assert!(versions::ensure_writable(&m.env, &m.env.current_link(), "add").is_err());
    }
}
