//! `poste sys setup`: builds, shows the plan, asks for confirmation, installs.

use crate::config::{self, Decision};
use crate::manifest::{Entry, Manifest};
use crate::plan::{self, Action, Item};
use crate::util::sha256_file;
use crate::{lines, orphans, units};
use anyhow::{bail, Context, Result};
use poste_core::{time, Env};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Options de `poste sys setup`.
#[derive(Debug, Default)]
pub struct Opts {
    pub yes: bool,
    pub dry_run: bool,
    /// Does not deal with the config (binaries only).
    pub no_config: bool,
    /// For tests: does not run `cargo build`.
    pub skip_build: bool,
    /// Dry run: the root read is not yet `current` (link not put in place); the config
    /// links are compared to `<link_root>/config/<path>`, the ones the real `setup` would put in place.
    pub link_root: Option<PathBuf>,
    pub take_repo: Vec<String>,
    pub take_home: Vec<String>,
}

fn decisions(env: &Env, o: &Opts) -> Result<HashMap<String, Decision>> {
    let mut map = HashMap::new();
    for (list, d) in [
        (&o.take_repo, Decision::Repo),
        (&o.take_home, Decision::Home),
    ] {
        for a in list {
            let rel = config::rel_of(env, a)?;
            if map.insert(rel.clone(), d).is_some_and(|prev| prev != d) {
                bail!("{rel}: --take-repo and --take-home together, choose one");
            }
        }
    }
    Ok(map)
}

/// Result of the binaries + config phase.
struct Phase {
    code: ExitCode,
    /// False if we stop there (refusal, cancellation).
    proceed: bool,
    /// True if config elements were put in place.
    config_changed: bool,
    /// Modules whose files are put in place or updated by this setup (their units restart).
    changed_modules: std::collections::HashSet<String>,
}

/// What `run` did: exit code, and whether the config put in place changed (island and Hyprland to reload).
pub struct Outcome {
    pub code: ExitCode,
    pub config_changed: bool,
}

/// `config::load`, with the links seen as the real `setup` would put them in place.
fn load_config(repo: &Path, env: &Env, o: &Opts) -> Result<Option<config::Layout>> {
    let mut layout = config::load(repo, env)?;
    if let (Some(l), Some(root)) = (&mut layout, &o.link_root) {
        l.relink(root);
    }
    Ok(layout)
}

/// Names of the modules and add-ons plugged in on this machine, as the repository sees it now.
fn plugged(repo: &Path, env: &Env) -> Result<std::collections::BTreeSet<String>> {
    Ok(plan::enabled_modules(repo, env)?
        .into_iter()
        .map(|m| m.name)
        .collect())
}

pub fn run(env: &Env, repo: &Path, o: &Opts) -> Result<Outcome> {
    let before = plugged(repo, env)?;
    let ph = install_phase(env, repo, o)?;
    if !ph.proceed || o.no_config {
        return Ok(Outcome {
            code: ph.code,
            config_changed: false,
        });
    }
    let extras = extras_phase(env, repo, o, &ph.changed_modules)?;
    println!("\n== Machine (bootstrap/) ==");
    let applied = crate::provision::setup(env, repo, o.yes, o.dry_run, &confirm)?;
    let mut code = ph.code;
    if extras.conflicts > 0 {
        code = ExitCode::from(2);
    }
    // `omarchy plugin add` may touch linked config files (shell.json):
    // the config is planned again and the links of the repository are put back.
    if !applied.done.is_empty() {
        if let Some(l) = load_config(repo, env, o)? {
            let p = config::plan(&l, env, &decisions(env, o)?)?;
            if p.writes() > 0 || !p.conflicts().is_empty() {
                println!("\nConfig to put back after the user steps:");
                config::print_plan(env, &p);
                let mut manifest = Manifest::load(&env.manifest_file())?;
                let stamp = time::now_compact();
                config::apply(&l, env, &p, &stamp, &mut manifest, &env.manifest_file())?;
                config::record_linked(&p, &mut manifest)?;
                manifest.save(&env.manifest_file())?;
                if !p.conflicts().is_empty() {
                    config::print_conflicts(env, &p);
                    code = ExitCode::from(2);
                }
            }
        }
    }
    // An add-on whose program a bootstrap step has just installed is plugged in now, so a
    // single setup is enough the first time.
    let mut plugged_now = false;
    if !o.dry_run {
        let new: Vec<String> = plugged(repo, env)?.difference(&before).cloned().collect();
        if !new.is_empty() {
            println!("\n== Now plugged in: {} ==", new.join(", "));
            let again = Opts {
                yes: o.yes,
                dry_run: false,
                no_config: false,
                skip_build: true,
                link_root: o.link_root.clone(),
                take_repo: o.take_repo.clone(),
                take_home: o.take_home.clone(),
            };
            let ph2 = install_phase(env, repo, &again)?;
            if ph2.code != ExitCode::SUCCESS && code == ExitCode::SUCCESS {
                code = ph2.code;
            }
            if ph2.proceed {
                let extras2 = extras_phase(env, repo, &again, &ph2.changed_modules)?;
                if extras2.conflicts > 0 {
                    code = ExitCode::from(2);
                }
                plugged_now =
                    ph2.config_changed || extras2.changed || !ph2.changed_modules.is_empty();
            }
        }
    }
    if !applied.failed.is_empty() {
        eprintln!("\nposte sys: step(s) failed: {}", applied.failed.join(", "));
        code = ExitCode::from(1);
    }
    Ok(Outcome {
        code,
        config_changed: (ph.config_changed || extras.changed || plugged_now) && !o.dry_run,
    })
}

/// Result of the "module lines and orphans" phase.
struct Extras {
    conflicts: usize,
    changed: bool,
}

/// Lines put in files without includes (`[[line]]` of the modules) and removal of
/// orphans (links whose source is gone, module files that are gone). A preview,
/// then a single confirmation (unless `--yes`); `--dry-run` writes nothing.
fn extras_phase(
    env: &Env,
    repo: &Path,
    o: &Opts,
    changed_modules: &std::collections::HashSet<String>,
) -> Result<Extras> {
    let manifest_file = env.manifest_file();
    let mut manifest = Manifest::load(&manifest_file)?;
    let layout = load_config(repo, env, o)?;
    let items = plan::desired(repo, env)?;
    let orphan_list = orphans::find(repo, env, layout.as_ref(), &manifest, &items)?;
    let vanishing: std::collections::HashSet<std::path::PathBuf> =
        orphan_list.iter().map(|x| x.path.clone()).collect();
    let specs = lines::desired(repo, env)?;
    let steps = lines::plan(&specs, &manifest, &vanishing);
    let runner = units::Runner::from_env();
    let unit_specs = units::desired(repo, env)?;
    let unit_steps = units::plan(env, &runner, &unit_specs, &manifest, changed_modules);
    units::print_plan(env, &unit_steps);
    orphans::print(env, &orphan_list);
    lines::print_plan(env, &steps);
    let conflicts = lines::conflicts(&steps) + units::conflicts(&unit_steps);
    let writes = orphan_list.len()
        + steps.iter().filter(|s| s.writes()).count()
        + unit_steps.iter().filter(|s| s.writes()).count();
    if writes == 0 {
        return Ok(Extras {
            conflicts,
            changed: false,
        });
    }
    if o.dry_run {
        println!("\nDry run (--dry-run): orphans and lines not touched.");
        return Ok(Extras {
            conflicts,
            changed: false,
        });
    }
    if !o.yes && !confirm(&format!("\nApply these {writes} change(s)? [y/N] "))? {
        println!("Cancelled.");
        return Ok(Extras {
            conflicts,
            changed: false,
        });
    }
    let stamp = time::now_compact();
    // Units to remove first: their files will disappear with the module.
    units::apply_removals(env, &runner, &unit_steps, &mut manifest)?;
    manifest.save(&manifest_file)?;
    orphans::apply(env, repo, &orphan_list, &stamp, &mut manifest)?;
    manifest.save(&manifest_file)?;
    // The links removed, the lines are planned again (a linked file that is gone is recreated).
    let steps = lines::plan(&specs, &manifest, &std::collections::HashSet::new());
    let n = lines::apply(env, &steps, &stamp, &mut manifest)?;
    units::apply_additions(&runner, &unit_steps, &mut manifest)?;
    manifest.save(&manifest_file)?;
    println!(
        "Done: {} orphan(s) removed, {n} line(s) handled.",
        orphan_list.len()
    );
    Ok(Extras {
        conflicts: lines::conflicts(&steps) + units::conflicts(&unit_steps),
        changed: true,
    })
}

/// Reminds of the modules that the machine's config does not want installed.
fn print_disabled(env: &Env) -> Result<()> {
    let cfg = poste_core::Config::load(env)?;
    let off: Vec<&str> = cfg
        .disabled
        .iter()
        .map(String::as_str)
        .filter(|d| *d != "sys")
        .collect();
    if !off.is_empty() {
        println!(
            "Modules disabled on this machine (config.toml, disabled): {}",
            off.join(", ")
        );
    }
    Ok(())
}

fn install_phase(env: &Env, repo: &Path, o: &Opts) -> Result<Phase> {
    let (yes, dry_run) = (o.yes, o.dry_run);
    let decisions = decisions(env, o)?;
    if o.no_config && !decisions.is_empty() {
        bail!("--take-repo/--take-home make no sense with --no-config");
    }
    // An installed version arrives built (no Cargo.toml); the clone is built.
    if !o.skip_build && repo.join("Cargo.toml").is_file() {
        build(repo)?;
    }

    let manifest_file = env.manifest_file();
    let mut manifest = Manifest::load(&manifest_file)?;
    let items = plan::desired(repo, env)?;
    for it in &items {
        if !it.src.is_file() {
            bail!("binary or source file missing: {}", it.src.display());
        }
    }
    let mut steps: Vec<(Item, Action)> = Vec::new();
    for it in items {
        let a = plan::classify(&it, &manifest)?;
        steps.push((it, a));
    }

    let changed_modules: std::collections::HashSet<String> = steps
        .iter()
        .filter(|(_, a)| matches!(a, Action::Create | Action::Update { .. }))
        .map(|(it, _)| it.module.clone())
        .collect();
    print_disabled(env)?;
    print_plan(env, &steps);

    // Clean refusal: nothing at all is written if a foreign file is in the way of a binary.
    let conflicts: Vec<_> = steps
        .iter()
        .filter(|(_, a)| *a == Action::Conflict)
        .collect();
    if !conflicts.is_empty() {
        eprintln!();
        for (it, _) in &conflicts {
            eprintln!(
                "poste sys: refused: {} exists and was not put in place by poste sys. Move or delete it yourself, then run again.",
                env.display(&it.dest)
            );
        }
        return Ok(Phase {
            code: ExitCode::from(1),
            proceed: false,
            config_changed: false,
            changed_modules: changed_modules.clone(),
        });
    }

    // The config: a separate plan, whose conflicts do not stop the rest.
    let layout = if o.no_config {
        None
    } else {
        load_config(repo, env, o)?
    };
    let cfg_plan = match &layout {
        Some(l) => {
            let p = config::plan(l, env, &decisions)?;
            config::print_plan(env, &p);
            config::print_local(l, env)?;
            Some(p)
        }
        None => None,
    };
    let cfg_conflicts = cfg_plan.as_ref().map_or(0, |p| p.conflicts().len());
    let cfg_writes = cfg_plan.as_ref().map_or(0, |p| p.writes());
    let code = if cfg_conflicts > 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    };
    let end = |msg: &str| {
        if let Some(p) = &cfg_plan {
            config::print_conflicts(env, p);
        }
        if cfg_conflicts > 0 {
            println!("\n{msg} ({cfg_conflicts} config conflict(s) to decide, exit code 2).");
        } else {
            println!("\n{msg}");
        }
    };

    let to_write = steps
        .iter()
        .filter(|(_, a)| matches!(a, Action::Create | Action::Update { .. }))
        .count();
    let to_adopt = steps.iter().filter(|(_, a)| *a == Action::Adopt).count();

    if to_write == 0 && cfg_writes == 0 {
        let mut changed = false;
        if !dry_run {
            if to_adopt > 0 {
                for (it, a) in &steps {
                    if *a == Action::Adopt {
                        record(&mut manifest, it)?;
                    }
                }
                changed = true;
            }
            if let Some(p) = &cfg_plan {
                changed |= config::record_linked(p, &mut manifest)?;
            }
            if changed {
                manifest.save(&manifest_file)?;
            }
        }
        end("Everything is up to date");
        return Ok(Phase {
            code,
            proceed: true,
            config_changed: false,
            changed_modules: changed_modules.clone(),
        });
    }
    if dry_run {
        end("Dry run (--dry-run): nothing was written");
        return Ok(Phase {
            code,
            proceed: true,
            config_changed: false,
            changed_modules: changed_modules.clone(),
        });
    }
    if !yes
        && !confirm(&format!(
            "\nApply these {} change(s)? [y/N] ",
            to_write + cfg_writes
        ))?
    {
        println!("Cancelled.");
        return Ok(Phase {
            code: ExitCode::from(1),
            proceed: false,
            config_changed: false,
            changed_modules: changed_modules.clone(),
        });
    }

    let stamp = time::now_compact();
    for (it, action) in &steps {
        match action {
            Action::Create | Action::Update { .. } => {
                if matches!(action, Action::Update { .. }) {
                    let bak = crate::util::backup(env, &it.dest, &stamp)?;
                    println!("  backup: {}", env.display(&bak));
                }
                install(&it.src, &it.dest)?;
                record(&mut manifest, it)?;
                manifest.save(&manifest_file)?;
            }
            Action::Adopt => {
                record(&mut manifest, it)?;
                manifest.save(&manifest_file)?;
            }
            _ => {}
        }
    }
    let mut done_cfg = 0;
    if let (Some(l), Some(p)) = (&layout, &cfg_plan) {
        done_cfg = config::apply(l, env, p, &stamp, &mut manifest, &manifest_file)?;
        config::record_linked(p, &mut manifest)?;
        manifest.save(&manifest_file)?;
    }
    end(&format!(
        "Done: {to_write} file(s) put in place, {done_cfg} config element(s) handled"
    ));
    Ok(Phase {
        code,
        proceed: true,
        config_changed: done_cfg > 0,
        changed_modules: changed_modules.clone(),
    })
}

pub(crate) fn build(repo: &Path) -> Result<()> {
    println!("Build: cargo build --release ({})", repo.display());
    let status = Command::new("cargo")
        .args(["build", "--release", "--quiet"])
        .current_dir(repo)
        .status()
        .context("cannot run cargo")?;
    if !status.success() {
        bail!("the build failed");
    }
    Ok(())
}

fn print_plan(env: &Env, steps: &[(Item, Action)]) {
    println!("\nPlan:");
    for (it, a) in steps {
        let note = match a {
            Action::Update { by_hand: true } => "  (overwrites a manual edit, old file backed up)",
            Action::Adopt => "  (already identical, now tracked)",
            Action::Conflict => "  (foreign file, not put in place by poste sys)",
            _ => "",
        };
        println!(
            "  {:<9} {}  [{}]{note}",
            label_short(a),
            env.display(&it.dest),
            it.module
        );
    }
}

fn label_short(a: &Action) -> &'static str {
    match a {
        Action::Create => "created",
        Action::Update { .. } => "changed",
        Action::Unchanged | Action::Adopt => "unchanged",
        Action::Conflict => "REFUSED",
    }
}

pub(crate) fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_lowercase().as_str(),
        "o" | "oui" | "y" | "yes"
    ))
}

/// Atomic copy: neighbouring temporary file then rename (works even if
/// the destination is a running binary, including poste-sys itself).
fn install(src: &Path, dest: &Path) -> Result<()> {
    let parent = dest
        .parent()
        .context("destination without a parent folder")?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".poste-tmp-{}", std::process::id()));
    std::fs::copy(src, &tmp).with_context(|| format!("copy to {}", tmp.display()))?;
    if let Err(e) = std::fs::rename(&tmp, dest) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("installation de {}", dest.display()));
    }
    Ok(())
}

fn record(manifest: &mut Manifest, it: &Item) -> Result<()> {
    manifest.upsert(Entry {
        path: it.dest.to_string_lossy().into_owned(),
        sha256: sha256_file(&it.dest)?,
        module: it.module.clone(),
        installed_at: time::now_rfc3339(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// The tests only look at the exit code.
    fn run(env: &Env, repo: &Path, o: &Opts) -> Result<ExitCode> {
        super::run(env, repo, o).map(|d| d.code)
    }

    /// Fake repository (with an already built "binary") and temporary HOME.
    fn fixture() -> (tempfile::TempDir, PathBuf, Env) {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir_all(repo.join("target/release")).unwrap();
        fs::write(repo.join("target/release/poste"), "bin").unwrap();
        fs::create_dir_all(repo.join("config/.config/a")).unwrap();
        fs::write(repo.join("config/.config/a/fresh"), "n").unwrap();
        fs::write(repo.join("config/.config/a/clash"), "repo text").unwrap();
        let home = d.path().join("home");
        fs::create_dir_all(home.join(".config/a")).unwrap();
        fs::write(home.join(".config/a/clash"), "home text").unwrap();
        let env = Env::with_home(&home);
        (d, repo, env)
    }

    fn opts(yes: bool, dry_run: bool) -> Opts {
        Opts {
            yes,
            dry_run,
            skip_build: true,
            ..Default::default()
        }
    }

    fn code(c: ExitCode) -> String {
        format!("{c:?}")
    }

    #[test]
    fn dry_run_writes_nothing() {
        let (_d, repo, env) = fixture();
        let c = run(&env, &repo, &opts(true, true)).unwrap();
        assert_eq!(code(c), code(ExitCode::from(2)), "conflict: non-zero code");
        assert!(!env.home.join(".local").exists(), "nothing in .local");
        assert!(!env.home.join(".config/a/fresh").exists());
        assert_eq!(
            fs::read_to_string(env.home.join(".config/a/clash")).unwrap(),
            "home text"
        );
    }

    #[test]
    fn application_non_blocking_conflict_then_rerun() {
        let (_d, repo, env) = fixture();
        let c = run(&env, &repo, &opts(true, false)).unwrap();
        assert_eq!(code(c), code(ExitCode::from(2)));
        assert!(env.home.join(".config/a/fresh").is_symlink());
        assert!(env.bin_dir().join("poste").is_file());
        assert!(!env.home.join(".config/a/clash").is_symlink());
        let m = Manifest::load(&env.manifest_file()).unwrap();
        assert!(m
            .files
            .iter()
            .any(|e| e.module == "config" && e.path.ends_with(".config/a/fresh")));
        assert!(m.files.iter().any(|e| e.module == "poste"));
        // Decide the conflict then run again: up to date, code 0, manifest unchanged.
        let o = Opts {
            take_repo: vec![".config/a/clash".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        assert!(env.home.join(".config/a/clash").is_symlink());
        let before = fs::read_to_string(env.manifest_file()).unwrap();
        assert_eq!(
            code(run(&env, &repo, &opts(true, false)).unwrap()),
            code(ExitCode::SUCCESS)
        );
        assert_eq!(fs::read_to_string(env.manifest_file()).unwrap(), before);
        // The old version of the conflict is backed up.
        let b: Vec<_> = fs::read_dir(env.backup_dir()).unwrap().flatten().collect();
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn no_config_and_inconsistent_decisions() {
        let (_d, repo, env) = fixture();
        let o = Opts {
            no_config: true,
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        assert!(!env.home.join(".config/a/fresh").exists());
        let o = Opts {
            take_repo: vec!["x".into()],
            take_home: vec!["x".into()],
            ..opts(true, true)
        };
        assert!(run(&env, &repo, &o).is_err());
    }

    #[test]
    fn script_module_lines_and_disappearance() {
        let (_d, repo, env) = fixture();
        let m = repo.join("modules/rec");
        fs::create_dir_all(m.join("hypr")).unwrap();
        fs::write(m.join("poste-rec"), "#!/bin/sh\n").unwrap();
        fs::write(m.join("hypr/rec.lua"), "-- rec\n").unwrap();
        fs::write(
            m.join("module.toml"),
            "name = \"rec\"\n[[line]]\nfile = \"~/.config/x.conf\"\nline = \"k:p = {home}/rec\"\n",
        )
        .unwrap();
        let home = env.home.clone();
        fs::write(home.join(".config/x.conf"), "mine = 1\n").unwrap();
        let o = Opts {
            take_repo: vec![".config/a/clash".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        let prog = env.modules_dir().join("rec/poste-rec");
        assert!(prog.is_file() && env.modules_dir().join("rec/hypr/rec.lua").is_file());
        let conf = fs::read_to_string(home.join(".config/x.conf")).unwrap();
        assert!(conf.starts_with("mine = 1\n") && conf.contains("k:p = "));
        let before = fs::read_to_string(env.manifest_file()).unwrap();
        run(&env, &repo, &opts(true, false)).unwrap();
        assert_eq!(fs::read_to_string(env.manifest_file()).unwrap(), before);
        // The module disappears from the repository: its files and its line go, the rest is intact.
        fs::remove_dir_all(&m).unwrap();
        run(&env, &repo, &opts(true, false)).unwrap();
        assert!(!env.modules_dir().join("rec").exists());
        assert_eq!(
            fs::read_to_string(home.join(".config/x.conf")).unwrap(),
            "mine = 1\n"
        );
        let man = Manifest::load(&env.manifest_file()).unwrap();
        assert!(man.lines.is_empty() && man.files.iter().all(|e| e.module != "rec"));
    }

    fn module_rec(repo: &Path) -> PathBuf {
        let m = repo.join("modules/rec");
        fs::create_dir_all(m.join("hypr")).unwrap();
        fs::create_dir_all(m.join("island")).unwrap();
        fs::create_dir_all(m.join("systemd")).unwrap();
        fs::write(m.join("poste-rec"), "#!/bin/sh\n").unwrap();
        fs::write(m.join("hypr/rec.lua"), "-- rec\n").unwrap();
        fs::write(m.join("island/launcher.json"), "{}\n").unwrap();
        fs::write(
            m.join("systemd/poste-rec.service"),
            "[Service]\nExecStart=/bin/true\n",
        )
        .unwrap();
        fs::write(
            m.join("module.toml"),
            "name = \"rec\"\nunits = [\"poste-rec.service\"]\n[[line]]\nfile = \"~/.config/x.conf\"\nline = \"k:p = {home}/rec\"\n",
        )
        .unwrap();
        m
    }

    #[test]
    fn local_files_never_touched() {
        let (_d, repo, env) = fixture();
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "local = [\".config/a/clash\"]\n").unwrap();
        // The different file is no longer a conflict: code 0, file intact, the rest is linked.
        assert_eq!(
            code(run(&env, &repo, &opts(true, false)).unwrap()),
            code(ExitCode::SUCCESS)
        );
        assert!(env.home.join(".config/a/fresh").is_symlink());
        let loc = env.home.join(".config/a/clash");
        assert!(!loc.is_symlink());
        assert_eq!(fs::read_to_string(&loc).unwrap(), "home text");
        let m = Manifest::load(&env.manifest_file()).unwrap();
        assert!(m.files.iter().all(|e| !e.path.ends_with("a/clash")));
        // The diff of a local path still shows the difference.
        let l = config::load(&repo, &env).unwrap().unwrap();
        assert_eq!(l.local.len(), 1);
        assert!(config::diff(&l, &env, Some(".config/a/clash")).unwrap());
        // Without difference diagnostic with no argument for the local ones.
        assert!(!config::diff(&l, &env, None).unwrap());
    }

    #[test]
    fn local_file_already_linked_stays_as_is() {
        let (_d, repo, env) = fixture();
        run(
            &env,
            &repo,
            &Opts {
                take_repo: vec![".config/a/clash".into()],
                ..opts(true, false)
            },
        )
        .unwrap();
        assert!(env.home.join(".config/a/clash").is_symlink());
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "local = [\".config/a\"]\n").unwrap();
        // A whole folder as local: nothing is created or removed any more.
        let before = fs::read_to_string(env.manifest_file()).unwrap();
        run(&env, &repo, &opts(true, false)).unwrap();
        assert!(env.home.join(".config/a/clash").is_symlink());
        assert_eq!(fs::read_to_string(env.manifest_file()).unwrap(), before);
    }

    #[test]
    fn disabled_module_removed_then_enabled_again() {
        let (_d, repo, env) = fixture();
        let (fake, log) = crate::units::tests::fake_systemctl(_d.path(), &env.home);
        std::env::set_var("POSTE_SYS_TEST_SYSTEMCTL", &fake);
        module_rec(&repo);
        let home = env.home.clone();
        fs::write(home.join(".config/x.conf"), "mine = 1\n").unwrap();
        let o = Opts {
            take_repo: vec![".config/a/clash".into()],
            ..opts(true, false)
        };
        run(&env, &repo, &o).unwrap();
        let base = env.modules_dir().join("rec");
        assert!(base.join("poste-rec").is_file() && base.join("hypr/rec.lua").is_file());
        assert!(base.join("systemd/poste-rec.service").is_file());
        assert!(fs::read_to_string(&log).unwrap().contains("enable --now"));
        assert!(home
            .join(".config/systemd/user/poste-rec.service")
            .is_symlink());
        assert!(fs::read_to_string(home.join(".config/x.conf"))
            .unwrap()
            .contains("k:p"));

        // Disabled: the preview alone removes nothing.
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "disabled = [\"rec\"]\n").unwrap();
        run(&env, &repo, &opts(true, true)).unwrap();
        assert!(base.join("poste-rec").is_file());
        assert!(home
            .join(".config/systemd/user/poste-rec.service")
            .is_symlink());
        // For real: files, hypr/, island/, unit and line go; the rest is intact.
        run(&env, &repo, &opts(true, false)).unwrap();
        assert!(!base.exists());
        assert!(fs::read_to_string(&log)
            .unwrap()
            .contains("disable --now poste-rec.service"));
        assert!(fs::symlink_metadata(home.join(".config/systemd/user/poste-rec.service")).is_err());
        assert_eq!(
            fs::read_to_string(home.join(".config/x.conf")).unwrap(),
            "mine = 1\n"
        );
        let man = Manifest::load(&env.manifest_file()).unwrap();
        assert!(man.units.is_empty() && man.lines.is_empty());
        assert!(man.files.iter().all(|e| e.module != "rec"));
        assert!(home.join(".config/a/fresh").is_symlink());

        // Enabled again: everything comes back.
        fs::write(env.config_file(), "disabled = []\n").unwrap();
        run(&env, &repo, &opts(true, false)).unwrap();
        assert!(base.join("island/launcher.json").is_file());
        assert!(home
            .join(".config/systemd/user/poste-rec.service")
            .is_symlink());
        let man = Manifest::load(&env.manifest_file()).unwrap();
        assert_eq!(man.units.len(), 1);
    }

    #[test]
    fn addon_plugged_in_by_the_setup_that_installs_its_program() {
        let (_d, repo, env) = fixture();
        let a = repo.join("addons/tool");
        fs::create_dir_all(a.join("hypr")).unwrap();
        fs::create_dir_all(a.join("bootstrap")).unwrap();
        fs::write(a.join("module.toml"), "requires = \"tool-prog-xyz\"\n").unwrap();
        fs::write(a.join("poste-tool"), "#!/bin/sh\n").unwrap();
        fs::write(a.join("hypr/tool.lua"), "-- key\n").unwrap();
        fs::write(
            a.join("bootstrap/steps.toml"),
            "[[user]]\nid = \"tool\"\ntitle = \"tool\"\ncheck = [\"test\", \"-x\", \"@HOME@/.local/bin/tool-prog-xyz\"]\nrun = [[\"sh\", \"-c\", \"mkdir -p @HOME@/.local/bin && printf '#!/bin/sh\\n' > @HOME@/.local/bin/tool-prog-xyz && chmod +x @HOME@/.local/bin/tool-prog-xyz\"]]\n",
        )
        .unwrap();
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "addons = [\"tool\"]\n").unwrap();
        let o = Opts {
            take_repo: vec![".config/a/clash".into()],
            ..opts(true, false)
        };
        run(&env, &repo, &o).unwrap();
        assert!(env.bin_dir().join("tool-prog-xyz").is_file(), "step ran");
        let base = env.modules_dir().join("tool");
        assert!(
            base.join("hypr/tool.lua").is_file(),
            "plugged in by the same setup"
        );
        assert!(base.join("poste-tool").is_file());
    }

    const SJ: &str = "{\n  \"bar\": {\"x\": 1},\n  \"idle\": {\n    \"lock\": 5400,\n    \"screensaver\": 2700\n  }\n}\n";

    fn with_override(repo: &Path, env: &Env) -> PathBuf {
        fs::write(repo.join("config/.config/a/s.json"), SJ).unwrap();
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(
            env.config_file(),
            "[[override]]\npath = \".config/a/s.json\"\nset = { \"idle.lock\" = 3600 }\n",
        )
        .unwrap();
        env.home.join(".config/a/s.json")
    }

    #[test]
    fn generated_json_override_regenerated_and_conflict() {
        let (_d, repo, env) = fixture();
        let dest = with_override(&repo, &env);
        let o = Opts {
            take_repo: vec![".config/a/clash".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        // Ordinary file (not a link), repository + override, the rest of the file follows the repository.
        assert!(!dest.is_symlink() && dest.is_file());
        let t = fs::read_to_string(&dest).unwrap();
        assert!(t.contains("\"lock\": 3600") && t.contains("\"screensaver\": 2700"));
        let man = Manifest::load(&env.manifest_file()).unwrap();
        assert!(man
            .files
            .iter()
            .any(|e| e.module == "config" && e.path.ends_with("a/s.json")));
        // Stable.
        let before = fs::read_to_string(env.manifest_file()).unwrap();
        run(&env, &repo, &opts(true, false)).unwrap();
        assert_eq!(fs::read_to_string(env.manifest_file()).unwrap(), before);

        // The repository changes: regenerated without conflict, override kept.
        fs::write(
            repo.join("config/.config/a/s.json"),
            SJ.replace("\"x\": 1", "\"x\": 2"),
        )
        .unwrap();
        assert_eq!(
            code(run(&env, &repo, &opts(true, false)).unwrap()),
            code(ExitCode::SUCCESS)
        );
        let t = fs::read_to_string(&dest).unwrap();
        assert!(t.contains("\"x\": 2") && t.contains("\"lock\": 3600"));

        // Edited by hand: conflict (code 2), file intact, status reports it.
        fs::write(&dest, t.replace("\"x\": 2", "\"x\": 9")).unwrap();
        assert_eq!(
            code(run(&env, &repo, &opts(true, false)).unwrap()),
            code(ExitCode::from(2))
        );
        assert!(fs::read_to_string(&dest).unwrap().contains("\"x\": 9"));
        let l = config::load(&repo, &env).unwrap().unwrap();
        let e = l
            .entries
            .iter()
            .find(|e| e.rel == ".config/a/s.json")
            .unwrap();
        assert_eq!(config::state_of(e, &env).unwrap(), config::State::Differs);
        assert!(config::diff(&l, &env, Some(".config/a/s.json")).unwrap());

        // --take-repo regenerates (old one backed up).
        let o = Opts {
            take_repo: vec![".config/a/s.json".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        assert!(fs::read_to_string(&dest).unwrap().contains("\"x\": 2"));
        assert!(fs::read_dir(env.backup_dir())
            .unwrap()
            .flatten()
            .any(|b| b.file_name().to_string_lossy().contains("s.json")));
    }

    #[test]
    fn override_take_home_restores_repository_values_or_refuses() {
        let (_d, repo, env) = fixture();
        let dest = with_override(&repo, &env);
        let o = Opts {
            take_repo: vec![".config/a/clash".into()],
            ..opts(true, false)
        };
        run(&env, &repo, &o).unwrap();
        let t = fs::read_to_string(&dest).unwrap();
        // Setting made by hand in ~: taken back into the repository, without the overridden value.
        fs::write(&dest, t.replace("\"x\": 1", "\"x\": 7")).unwrap();
        let o = Opts {
            take_home: vec![".config/a/s.json".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        let r = fs::read_to_string(repo.join("config/.config/a/s.json")).unwrap();
        assert!(
            r.contains("\"x\": 7") && r.contains("\"lock\": 5400"),
            "{r}"
        );
        let d = fs::read_to_string(&dest).unwrap();
        assert!(d.contains("\"x\": 7") && d.contains("\"lock\": 3600"));
        // Overridden value changed by hand: ambiguous take-back, refusal, nothing written.
        fs::write(&dest, d.replace("3600", "1800")).unwrap();
        let repo_before = fs::read_to_string(repo.join("config/.config/a/s.json")).unwrap();
        assert!(run(&env, &repo, &o).is_err());
        assert_eq!(
            fs::read_to_string(repo.join("config/.config/a/s.json")).unwrap(),
            repo_before
        );
        assert!(fs::read_to_string(&dest).unwrap().contains("1800"));
    }

    #[test]
    fn override_replaces_a_link_and_adopts_an_equivalent_file() {
        let (_d, repo, env) = fixture();
        let dest = with_override(&repo, &env);
        // Old link to the repository (before the override): replaced by a generated file.
        std::os::unix::fs::symlink(repo.join("config/.config/a/s.json"), &dest).unwrap();
        let o = Opts {
            take_repo: vec![".config/a/clash".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        assert!(!dest.is_symlink());
        // The work PC: same values written by hand, other formatting: adopted without conflict.
        fs::write(
            &dest,
            "{\"bar\":{\"x\":1},\"idle\":{\"lock\":3600,\"screensaver\":2700}}",
        )
        .unwrap();
        assert_eq!(
            code(run(&env, &repo, &opts(true, false)).unwrap()),
            code(ExitCode::SUCCESS)
        );
        assert!(fs::read_to_string(&dest).unwrap().contains("  \"idle\": {"));
        // Dry run: nothing is written.
        fs::write(
            repo.join("config/.config/a/s.json"),
            SJ.replace("\"x\": 1", "\"x\": 3"),
        )
        .unwrap();
        let cur = fs::read_to_string(&dest).unwrap();
        run(&env, &repo, &opts(true, true)).unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), cur);
    }
}
