//! Systemd user units provided by the modules.
//!
//! A module declares `units = ["poste-power.service"]` in `module.toml` and puts the files
//! in `systemd/` (copied with the rest to `~/.local/share/poste/modules/<name>/systemd/`).
//! `setup` enables them with `systemctl --user enable --now <path of the file>` (systemd creates the
//! link in `~/.config/systemd/user/`; nothing is copied elsewhere) and notes the unit in
//! `installed.json` (`units`). If the module disappears, is disabled or no longer declares the unit,
//! `setup` disables it (`disable --now`) BEFORE removing the files of the module.
//!
//! A timer (`x.timer`) triggers the service of the same name (`x.service`), put next to it in
//! `systemd/` but not declared in `units`: `setup` first links it (`systemctl --user link`,
//! link in `~/.config/systemd/user/`) so that systemd finds it, then enables the timer. On
//! removal, the linked service is unlinked (`disable`) after the timer.
//!
//! Never touched: a unit of the same name that we did not put in place (ordinary file or link
//! elsewhere in `~/.config/systemd/user/`): conflict reported.

use crate::manifest::{Manifest, UnitEntry};
use crate::plan;
use anyhow::{bail, Result};
use poste_core::{time, Card, Env};
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The `systemctl` program; replaceable by `POSTE_SYS_TEST_SYSTEMCTL` (RESERVED FOR TESTS).
#[derive(Debug, Clone)]
pub struct Runner {
    program: OsString,
}

impl Runner {
    pub fn from_env() -> Self {
        Self::with_program(
            std::env::var_os("POSTE_SYS_TEST_SYSTEMCTL")
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "systemctl".into()),
        )
    }

    pub fn with_program(p: impl Into<OsString>) -> Self {
        Self { program: p.into() }
    }

    /// `systemctl --user <args>`; true if the exit code is 0.
    fn run(&self, args: &[&str]) -> bool {
        Command::new(&self.program)
            .arg("--user")
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    fn is_enabled(&self, unit: &str) -> bool {
        self.run(&["is-enabled", "--quiet", unit])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub module: String,
    pub unit: String,
    /// Installed file: `~/.local/share/poste/modules/<module>/systemd/<unit>`.
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Enable and start.
    Enable,
    /// Already active; the module has just been updated: reload and restart.
    Restart,
    Unchanged,
    /// Already enabled through the right link but not yet noted: only record it.
    Adopt,
    /// No longer wanted: disable and stop.
    Disable,
    /// No longer wanted, already disabled: forget the entry.
    Forget,
    Conflict(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub op: Op,
    pub module: String,
    pub unit: String,
    pub file: PathBuf,
}

impl Step {
    pub fn writes(&self) -> bool {
        !matches!(self.op, Op::Unchanged | Op::Conflict(_))
    }
    /// Removal: to do before removing the files of the module.
    pub fn removes(&self) -> bool {
        matches!(self.op, Op::Disable | Op::Forget)
    }
}

/// The service triggered by a timer: `x.timer` → `x.service`.
pub fn companion(unit: &str) -> Option<String> {
    unit.strip_suffix(".timer").map(|n| format!("{n}.service"))
}

fn valid_unit(unit: &str) -> bool {
    !unit.contains('/')
        && !unit.starts_with('-')
        && [".service", ".timer", ".path", ".socket"]
            .iter()
            .any(|e| unit.ends_with(e))
}

/// The wanted units: those of the enabled modules of the repository (source file required).
pub fn desired(repo: &Path, env: &Env) -> Result<Vec<Spec>> {
    let mut out = Vec::new();
    for m in plan::enabled_modules(repo, env)? {
        let card = Card::load(&m.dir.join("module.toml"))?;
        for unit in card.units {
            if !valid_unit(&unit) {
                bail!(
                    "module {}: unit '{unit}' invalid (expected: name.service, name.timer…)",
                    m.name
                );
            }
            if !m.dir.join("systemd").join(&unit).is_file() {
                bail!("module {}: modules/{0}/systemd/{unit} is missing", m.name);
            }
            if let Some(svc) = companion(&unit) {
                if !m.dir.join("systemd").join(&svc).is_file() {
                    bail!(
                        "module {}: the timer {unit} needs modules/{0}/systemd/{svc}",
                        m.name
                    );
                }
            }
            out.push(Spec {
                file: env.modules_dir().join(&m.name).join("systemd").join(&unit),
                module: m.name.clone(),
                unit,
            });
        }
    }
    Ok(out)
}

fn link_path(env: &Env, unit: &str) -> PathBuf {
    env.config_home.join("systemd/user").join(unit)
}

/// Does systemd's activation link point to a module file that we put in place?
fn link_is_ours(env: &Env, link: &Path) -> bool {
    std::fs::read_link(link)
        .map(|t| t.starts_with(env.modules_dir()))
        .unwrap_or(false)
}

pub fn plan(
    env: &Env,
    runner: &Runner,
    specs: &[Spec],
    manifest: &Manifest,
    changed_modules: &HashSet<String>,
) -> Vec<Step> {
    let mut steps = Vec::new();
    for s in specs {
        let link = link_path(env, &s.unit);
        let tracked = manifest.units.iter().any(|u| u.unit == s.unit);
        let op = match std::fs::symlink_metadata(&link) {
            Err(_) => Op::Enable,
            Ok(m)
                if m.file_type().is_symlink()
                    && std::fs::read_link(&link).ok().as_deref() == Some(s.file.as_path()) =>
            {
                if !runner.is_enabled(&s.unit) {
                    Op::Enable
                } else if changed_modules.contains(&s.module) {
                    Op::Restart
                } else if tracked {
                    Op::Unchanged
                } else {
                    Op::Adopt
                }
            }
            Ok(_) => Op::Conflict(format!(
                "{} exists and was not put in place by poste sys",
                env.display(&link)
            )),
        };
        let op = match companion(&s.unit) {
            Some(svc) if !matches!(op, Op::Conflict(_)) => {
                let svc_link = link_path(env, &svc);
                let svc_file = s.file.with_file_name(&svc);
                match std::fs::symlink_metadata(&svc_link) {
                    Err(_) => Op::Enable,
                    Ok(m)
                        if m.file_type().is_symlink()
                            && std::fs::read_link(&svc_link).ok().as_deref()
                                == Some(svc_file.as_path()) =>
                    {
                        op
                    }
                    Ok(_) => Op::Conflict(format!(
                        "{} exists and was not put in place by poste sys",
                        env.display(&svc_link)
                    )),
                }
            }
            _ => op,
        };
        steps.push(Step {
            op,
            module: s.module.clone(),
            unit: s.unit.clone(),
            file: s.file.clone(),
        });
    }
    for u in &manifest.units {
        if specs.iter().any(|s| s.unit == u.unit) {
            continue;
        }
        let link = link_path(env, &u.unit);
        let op = if link_is_ours(env, &link) {
            Op::Disable
        } else {
            Op::Forget
        };
        steps.push(Step {
            op,
            module: u.module.clone(),
            unit: u.unit.clone(),
            file: link,
        });
    }
    steps
}

pub fn label(op: &Op) -> &'static str {
    match op {
        Op::Enable => "to enable",
        Op::Restart => "to restart",
        Op::Unchanged => "active",
        Op::Adopt => "to note",
        Op::Disable => "to disable",
        Op::Forget => "to forget",
        Op::Conflict(_) => "conflict",
    }
}

pub fn print_plan(env: &Env, steps: &[Step]) {
    let shown: Vec<_> = steps
        .iter()
        .filter(|s| !matches!(s.op, Op::Unchanged))
        .collect();
    if shown.is_empty() {
        return;
    }
    println!("\nSystemd user units:");
    for s in shown {
        println!("  {:<13} {}  [{}]", label(&s.op), s.unit, s.module);
        if let Op::Conflict(why) = &s.op {
            println!("                {why}");
        }
        if s.op == Op::Enable {
            if let Some(svc) = companion(&s.unit) {
                println!(
                    "                systemctl --user link {}",
                    env.display(&s.file.with_file_name(svc))
                );
            }
            println!(
                "                systemctl --user enable --now {}",
                env.display(&s.file)
            );
        }
    }
}

pub fn conflicts(steps: &[Step]) -> usize {
    steps
        .iter()
        .filter(|s| matches!(s.op, Op::Conflict(_)))
        .count()
}

/// Disables the units that are no longer wanted (before the files of the module are removed).
pub fn apply_removals(
    env: &Env,
    runner: &Runner,
    steps: &[Step],
    manifest: &mut Manifest,
) -> Result<usize> {
    let mut n = 0;
    let mut reload = false;
    for s in steps.iter().filter(|s| s.removes()) {
        if s.op == Op::Disable {
            if !runner.run(&["disable", "--now", &s.unit]) {
                eprintln!(
                    "poste sys: systemctl --user disable --now {} failed, unit kept in the list",
                    s.unit
                );
                continue;
            }
            // `disable` normally removes the link; otherwise we remove ours.
            let link = link_path(env, &s.unit);
            if link_is_ours(env, &link) {
                let _ = std::fs::remove_file(&link);
            }
            reload = true;
        }
        // The linked service of a timer, if it is ours.
        if let Some(svc) = companion(&s.unit) {
            let svc_link = link_path(env, &svc);
            if link_is_ours(env, &svc_link) {
                runner.run(&["disable", &svc]);
                if link_is_ours(env, &svc_link) {
                    let _ = std::fs::remove_file(&svc_link);
                }
                reload = true;
            }
        }
        manifest.units.retain(|u| u.unit != s.unit);
        n += 1;
    }
    if reload {
        runner.run(&["daemon-reload"]);
    }
    Ok(n)
}

/// Enables the wanted units and restarts those whose module has just been updated.
pub fn apply_additions(runner: &Runner, steps: &[Step], manifest: &mut Manifest) -> Result<usize> {
    let mut n = 0;
    for s in steps {
        let ok = match s.op {
            Op::Enable => {
                // The service of a timer first, otherwise systemd does not find it.
                let linked = match companion(&s.unit) {
                    Some(svc) => {
                        runner.run(&["link", &s.file.with_file_name(svc).to_string_lossy()])
                    }
                    None => true,
                };
                runner.run(&["daemon-reload"]);
                let enabled = linked && runner.run(&["enable", "--now", &s.file.to_string_lossy()]);
                // Timer refused: the link of the service must not stay, nothing would remove it
                // (the unit does not enter the manifest).
                if !enabled && linked {
                    if let Some(svc) = companion(&s.unit) {
                        runner.run(&["disable", &svc]);
                    }
                }
                enabled
            }
            Op::Restart => {
                runner.run(&["daemon-reload"]);
                runner.run(&["try-restart", &s.unit])
            }
            Op::Adopt => true,
            _ => continue,
        };
        if !ok {
            eprintln!(
                "poste sys: the unit {} could not be enabled (systemctl --user)",
                s.unit
            );
            continue;
        }
        if !manifest.units.iter().any(|u| u.unit == s.unit) {
            manifest.units.push(UnitEntry {
                unit: s.unit.clone(),
                module: s.module.clone(),
                installed_at: time::now_rfc3339(),
            });
        }
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    struct Fx {
        _d: tempfile::TempDir,
        env: Env,
        runner: Runner,
        log: PathBuf,
    }

    /// Fake systemctl: logs its arguments; `enable` creates the activation link and a
    /// marker, `disable` removes them, `is-enabled` reads the marker.
    pub(crate) fn fake_systemctl(dir: &Path, home: &Path) -> (PathBuf, PathBuf) {
        let log = dir.join("calls.log");
        let marks = dir.join("marks");
        fs::create_dir_all(&marks).unwrap();
        let script = dir.join("fake-systemctl");
        fs::write(
            &script,
            format!(
                r#"#!/bin/sh
shift
echo "$@" >> '{log}'
for a; do last="$a"; done
case "$1" in
  enable) f="$3"; u=$(basename "$f"); mkdir -p '{home}/.config/systemd/user'; ln -sfn "$f" "{home}/.config/systemd/user/$u"; touch '{marks}'/"$u" ;;
  link) f="$2"; mkdir -p '{home}/.config/systemd/user'; ln -sfn "$f" "{home}/.config/systemd/user/$(basename "$f")" ;;
  disable) rm -f '{home}/.config/systemd/user'/"$last" '{marks}'/"$last" ;;
  is-enabled) [ -e '{marks}'/"$3" ] ;;
esac
"#,
                log = log.display(),
                home = home.display(),
                marks = marks.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        (script, log)
    }

    fn fx() -> Fx {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        fs::create_dir_all(&home).unwrap();
        let (script, log) = fake_systemctl(d.path(), &home);
        Fx {
            env: Env::with_home(&home),
            runner: Runner::with_program(script),
            log,
            _d: d,
        }
    }

    fn spec(f: &Fx, module: &str, unit: &str) -> Spec {
        let file = f.env.modules_dir().join(module).join("systemd").join(unit);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "[Service]\nExecStart=/bin/true\n").unwrap();
        Spec {
            module: module.into(),
            unit: unit.into(),
            file,
        }
    }

    fn calls(f: &Fx) -> String {
        fs::read_to_string(&f.log).unwrap_or_default()
    }

    #[test]
    fn activation_then_stable_then_restart_then_removal() {
        let f = fx();
        let s = spec(&f, "power", "poste-power.service");
        let mut m = Manifest::default();
        let none = HashSet::new();

        let st = plan(&f.env, &f.runner, std::slice::from_ref(&s), &m, &none);
        assert_eq!(st[0].op, Op::Enable);
        apply_additions(&f.runner, &st, &mut m).unwrap();
        assert!(calls(&f).contains(&format!("enable --now {}", s.file.display())));
        assert_eq!(m.units.len(), 1);

        let st = plan(&f.env, &f.runner, std::slice::from_ref(&s), &m, &none);
        assert_eq!(st[0].op, Op::Unchanged);

        let changed: HashSet<String> = ["power".to_string()].into();
        let st = plan(&f.env, &f.runner, std::slice::from_ref(&s), &m, &changed);
        assert_eq!(st[0].op, Op::Restart);
        apply_additions(&f.runner, &st, &mut m).unwrap();
        assert!(calls(&f).contains("try-restart poste-power.service"));

        // The module is disabled: the unit is no longer wanted.
        let st = plan(&f.env, &f.runner, &[], &m, &none);
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].op, Op::Disable);
        apply_removals(&f.env, &f.runner, &st, &mut m).unwrap();
        assert!(calls(&f).contains("disable --now poste-power.service"));
        assert!(m.units.is_empty());
        assert!(fs::symlink_metadata(link_path(&f.env, "poste-power.service")).is_err());
    }

    #[test]
    fn foreign_unit_in_conflict_and_never_touched() {
        let f = fx();
        let s = spec(&f, "power", "poste-power.service");
        let link = link_path(&f.env, "poste-power.service");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        fs::write(&link, "mine").unwrap();
        let m = Manifest::default();
        let st = plan(&f.env, &f.runner, &[s], &m, &HashSet::new());
        assert!(matches!(st[0].op, Op::Conflict(_)));
        assert_eq!(conflicts(&st), 1);
        assert_eq!(fs::read_to_string(&link).unwrap(), "mine");
        assert!(calls(&f).is_empty());
    }

    #[test]
    fn noted_entry_whose_link_vanished_is_forgotten() {
        let f = fx();
        let mut m = Manifest::default();
        m.units.push(UnitEntry {
            unit: "x.service".into(),
            module: "x".into(),
            installed_at: "t".into(),
        });
        let st = plan(&f.env, &f.runner, &[], &m, &HashSet::new());
        assert_eq!(st[0].op, Op::Forget);
        apply_removals(&f.env, &f.runner, &st, &mut m).unwrap();
        assert!(m.units.is_empty());
        assert!(calls(&f).is_empty());
    }

    #[test]
    fn timer_links_its_service_then_unlinks_it() {
        let f = fx();
        let t = spec(&f, "usage", "poste-usage-record.timer");
        let svc = spec(&f, "usage", "poste-usage-record.service").file;
        let mut m = Manifest::default();
        let none = HashSet::new();

        let st = plan(&f.env, &f.runner, std::slice::from_ref(&t), &m, &none);
        assert_eq!(st[0].op, Op::Enable);
        apply_additions(&f.runner, &st, &mut m).unwrap();
        let c = calls(&f);
        let link_at = c.find(&format!("link {}", svc.display())).unwrap();
        let enable_at = c
            .find(&format!("enable --now {}", t.file.display()))
            .unwrap();
        assert!(link_at < enable_at, "the service is linked before the timer");
        let svc_link = link_path(&f.env, "poste-usage-record.service");
        assert_eq!(fs::read_link(&svc_link).unwrap(), svc);

        let st = plan(&f.env, &f.runner, std::slice::from_ref(&t), &m, &none);
        assert_eq!(st[0].op, Op::Unchanged);

        // The service link has disappeared: the activation is redone.
        fs::remove_file(&svc_link).unwrap();
        let st = plan(&f.env, &f.runner, std::slice::from_ref(&t), &m, &none);
        assert_eq!(st[0].op, Op::Enable);
        apply_additions(&f.runner, &st, &mut m).unwrap();
        assert!(svc_link.is_symlink());

        let st = plan(&f.env, &f.runner, &[], &m, &none);
        assert_eq!(st[0].op, Op::Disable);
        apply_removals(&f.env, &f.runner, &st, &mut m).unwrap();
        assert!(calls(&f).contains("disable --now poste-usage-record.timer"));
        assert!(calls(&f).contains("disable poste-usage-record.service"));
        assert!(fs::symlink_metadata(&svc_link).is_err());
        assert!(fs::symlink_metadata(link_path(&f.env, "poste-usage-record.timer")).is_err());
        assert!(m.units.is_empty());
    }

    #[test]
    fn foreign_timer_service_in_conflict() {
        let f = fx();
        let t = spec(&f, "usage", "poste-usage-record.timer");
        spec(&f, "usage", "poste-usage-record.service");
        let svc_link = link_path(&f.env, "poste-usage-record.service");
        fs::create_dir_all(svc_link.parent().unwrap()).unwrap();
        fs::write(&svc_link, "mine").unwrap();
        let st = plan(
            &f.env,
            &f.runner,
            &[t],
            &Manifest::default(),
            &HashSet::new(),
        );
        assert!(matches!(st[0].op, Op::Conflict(_)));
        assert_eq!(fs::read_to_string(&svc_link).unwrap(), "mine");
    }

    #[test]
    fn unit_names() {
        assert_eq!(
            companion("poste-usage-record.timer").as_deref(),
            Some("poste-usage-record.service")
        );
        assert_eq!(companion("poste-power.service"), None);
        assert!(valid_unit("poste-power.service"));
        assert!(!valid_unit("../x.service"));
        assert!(!valid_unit("x"));
        assert!(!valid_unit("-x.service"));
    }
}
