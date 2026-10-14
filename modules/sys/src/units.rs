//! Unités systemd utilisateur fournies par les modules.
//!
//! Un module déclare `units = ["poste-power.service"]` dans `module.toml` et range les fichiers
//! dans `systemd/` (copiés avec le reste dans `~/.local/share/poste/modules/<nom>/systemd/`).
//! `setup` les active avec `systemctl --user enable --now <chemin du fichier>` (systemd crée le
//! lien dans `~/.config/systemd/user/` ; rien n'est copié ailleurs) et note l'unité dans
//! `installed.json` (`units`). Si le module disparaît, est désactivé ou ne déclare plus l'unité,
//! `setup` la désactive (`disable --now`) AVANT de retirer les fichiers du module.
//!
//! Un minuteur (`x.timer`) déclenche le service du même nom (`x.service`), rangé à côté dans
//! `systemd/` mais pas déclaré dans `units` : `setup` le lie d'abord (`systemctl --user link`,
//! lien dans `~/.config/systemd/user/`) pour que systemd le trouve, puis active le minuteur. Au
//! retrait, le service lié est délié (`disable`) après le minuteur.
//!
//! Jamais touché : une unité du même nom que nous n'avons pas posée (fichier ordinaire ou lien
//! ailleurs dans `~/.config/systemd/user/`) : conflit signalé.

use crate::manifest::{Manifest, UnitEntry};
use crate::plan;
use anyhow::{bail, Result};
use poste_core::{time, Card, Env};
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Le programme `systemctl` ; remplaçable par `POSTE_SYS_TEST_SYSTEMCTL` (RÉSERVÉ AUX TESTS).
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

    /// `systemctl --user <args>` ; vrai si le code de sortie est 0.
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
    /// Fichier installé : `~/.local/share/poste/modules/<module>/systemd/<unit>`.
    pub file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Activer et démarrer.
    Enable,
    /// Déjà active ; le module vient d'être mis à jour : recharger et redémarrer.
    Restart,
    Unchanged,
    /// Déjà activée par le bon lien mais pas encore notée : on l'enregistre seulement.
    Adopt,
    /// Plus voulue : désactiver et arrêter.
    Disable,
    /// Plus voulue, déjà désactivée : oublier l'entrée.
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
    /// Retrait : à faire avant de retirer les fichiers du module.
    pub fn removes(&self) -> bool {
        matches!(self.op, Op::Disable | Op::Forget)
    }
}

/// Le service déclenché par un minuteur : `x.timer` → `x.service`.
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

/// Les unités voulues : celles des modules activés du dépôt (fichier source obligatoire).
pub fn desired(repo: &Path, env: &Env) -> Result<Vec<Spec>> {
    let mut out = Vec::new();
    for m in plan::enabled_modules(repo, env)? {
        let card = Card::load(&m.dir.join("module.toml"))?;
        for unit in card.units {
            if !valid_unit(&unit) {
                bail!(
                    "module {} : unité « {unit} » invalide (attendu : nom.service, nom.timer…)",
                    m.name
                );
            }
            if !m.dir.join("systemd").join(&unit).is_file() {
                bail!("module {} : modules/{0}/systemd/{unit} est absent", m.name);
            }
            if let Some(svc) = companion(&unit) {
                if !m.dir.join("systemd").join(&svc).is_file() {
                    bail!(
                        "module {} : le minuteur {unit} demande modules/{0}/systemd/{svc}",
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

/// Le lien d'activation de systemd pointe-t-il vers un fichier de module posé par nous ?
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
                "{} existe et n'a pas été posé par poste sys",
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
                        "{} existe et n'a pas été posé par poste sys",
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
        Op::Enable => "à activer",
        Op::Restart => "à redémarrer",
        Op::Unchanged => "active",
        Op::Adopt => "à noter",
        Op::Disable => "à désactiver",
        Op::Forget => "à oublier",
        Op::Conflict(_) => "conflit",
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
    println!("\nUnités systemd utilisateur :");
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

/// Désactive les unités qui ne sont plus voulues (avant le retrait des fichiers du module).
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
                    "poste sys : systemctl --user disable --now {} a échoué, unité gardée dans la liste",
                    s.unit
                );
                continue;
            }
            // `disable` retire normalement le lien ; sinon on retire le nôtre.
            let link = link_path(env, &s.unit);
            if link_is_ours(env, &link) {
                let _ = std::fs::remove_file(&link);
            }
            reload = true;
        }
        // Le service lié d'un minuteur, s'il est à nous.
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

/// Active les unités voulues et redémarre celles dont le module vient d'être mis à jour.
pub fn apply_additions(runner: &Runner, steps: &[Step], manifest: &mut Manifest) -> Result<usize> {
    let mut n = 0;
    for s in steps {
        let ok = match s.op {
            Op::Enable => {
                // Le service d'un minuteur d'abord, sinon systemd ne le trouve pas.
                let linked = match companion(&s.unit) {
                    Some(svc) => {
                        runner.run(&["link", &s.file.with_file_name(svc).to_string_lossy()])
                    }
                    None => true,
                };
                runner.run(&["daemon-reload"]);
                let enabled = linked && runner.run(&["enable", "--now", &s.file.to_string_lossy()]);
                // Minuteur refusé : le lien du service ne doit pas rester, rien ne le retirerait
                // (l'unité n'entre pas dans le manifeste).
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
                "poste sys : l'unité {} n'a pas pu être activée (systemctl --user)",
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

    /// Faux systemctl : journalise ses arguments ; `enable` crée le lien d'activation et un
    /// marqueur, `disable` les retire, `is-enabled` lit le marqueur.
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
    fn activation_puis_stable_puis_redemarrage_puis_retrait() {
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

        // Le module est désactivé : l'unité n'est plus voulue.
        let st = plan(&f.env, &f.runner, &[], &m, &none);
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].op, Op::Disable);
        apply_removals(&f.env, &f.runner, &st, &mut m).unwrap();
        assert!(calls(&f).contains("disable --now poste-power.service"));
        assert!(m.units.is_empty());
        assert!(fs::symlink_metadata(link_path(&f.env, "poste-power.service")).is_err());
    }

    #[test]
    fn unite_etrangere_en_conflit_et_jamais_touchee() {
        let f = fx();
        let s = spec(&f, "power", "poste-power.service");
        let link = link_path(&f.env, "poste-power.service");
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        fs::write(&link, "à moi").unwrap();
        let m = Manifest::default();
        let st = plan(&f.env, &f.runner, &[s], &m, &HashSet::new());
        assert!(matches!(st[0].op, Op::Conflict(_)));
        assert_eq!(conflicts(&st), 1);
        assert_eq!(fs::read_to_string(&link).unwrap(), "à moi");
        assert!(calls(&f).is_empty());
    }

    #[test]
    fn entree_notee_dont_le_lien_a_disparu_est_oubliee() {
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
    fn minuteur_lie_son_service_puis_le_delie() {
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
        assert!(link_at < enable_at, "le service est lié avant le minuteur");
        let svc_link = link_path(&f.env, "poste-usage-record.service");
        assert_eq!(fs::read_link(&svc_link).unwrap(), svc);

        let st = plan(&f.env, &f.runner, std::slice::from_ref(&t), &m, &none);
        assert_eq!(st[0].op, Op::Unchanged);

        // Le lien du service a disparu : on refait l'activation.
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
    fn service_d_un_minuteur_etranger_en_conflit() {
        let f = fx();
        let t = spec(&f, "usage", "poste-usage-record.timer");
        spec(&f, "usage", "poste-usage-record.service");
        let svc_link = link_path(&f.env, "poste-usage-record.service");
        fs::create_dir_all(svc_link.parent().unwrap()).unwrap();
        fs::write(&svc_link, "à moi").unwrap();
        let st = plan(
            &f.env,
            &f.runner,
            &[t],
            &Manifest::default(),
            &HashSet::new(),
        );
        assert!(matches!(st[0].op, Op::Conflict(_)));
        assert_eq!(fs::read_to_string(&svc_link).unwrap(), "à moi");
    }

    #[test]
    fn noms_d_unite() {
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
