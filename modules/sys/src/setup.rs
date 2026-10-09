//! `poste sys setup` : construit, montre le plan, demande confirmation, installe.

use crate::config::{self, Decision};
use crate::manifest::{Entry, Manifest};
use crate::plan::{self, Action, Item};
use crate::util::sha256_file;
use crate::{lines, orphans, units};
use anyhow::{bail, Context, Result};
use poste_core::{time, Env};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Command, ExitCode};

/// Options de `poste sys setup`.
#[derive(Debug, Default)]
pub struct Opts {
    pub yes: bool,
    pub dry_run: bool,
    /// Ne s'occupe pas de la config (binaires seulement).
    pub no_config: bool,
    /// Pour les tests : ne lance pas `cargo build`.
    pub skip_build: bool,
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
                bail!("{rel} : --take-repo et --take-home ensemble, choisissez");
            }
        }
    }
    Ok(map)
}

/// Résultat de la phase binaires + config.
struct Phase {
    code: ExitCode,
    /// Faux si on s'arrête là (refus, annulation).
    proceed: bool,
    /// Vrai si des éléments de config ont été posés.
    config_changed: bool,
    /// Modules dont des fichiers sont posés ou mis à jour par ce setup (leurs unités redémarrent).
    changed_modules: std::collections::HashSet<String>,
}

pub fn run(env: &Env, repo: &Path, o: &Opts) -> Result<ExitCode> {
    let ph = install_phase(env, repo, o)?;
    if !ph.proceed || o.no_config {
        return Ok(ph.code);
    }
    let extras = extras_phase(env, repo, o, &ph.changed_modules)?;
    println!("\n== Machine (bootstrap/) ==");
    let applied = crate::provision::setup(env, repo, o.yes, o.dry_run, &confirm)?;
    let mut code = ph.code;
    if extras.conflicts > 0 {
        code = ExitCode::from(2);
    }
    // `omarchy plugin add` peut toucher à des fichiers de config liés (shell.json) :
    // on replanifie la config et on remet les liens du dépôt.
    if !applied.done.is_empty() {
        if let Some(l) = config::load(repo, env)? {
            let p = config::plan(&l, env, &decisions(env, o)?)?;
            if p.writes() > 0 || !p.conflicts().is_empty() {
                println!("\nConfig à remettre après les étapes utilisateur :");
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
    if !applied.failed.is_empty() {
        eprintln!(
            "\nposte sys : étape(s) en échec : {}",
            applied.failed.join(", ")
        );
        code = ExitCode::from(1);
    }
    if (ph.config_changed || extras.changed) && !o.dry_run {
        println!(
            "\nConfig modifiée : pour la recharger, lancez vous-même `omarchy restart shell` et `hyprctl reload`."
        );
    }
    Ok(code)
}

/// Résultat de la phase « lignes de module et orphelins ».
struct Extras {
    conflicts: usize,
    changed: bool,
}

/// Lignes posées dans des fichiers sans inclusion (`[[line]]` des modules) et retrait des
/// orphelins (liens dont la source a disparu, fichiers de modules disparus). Un aperçu,
/// puis une seule confirmation (sauf `--yes`) ; `--dry-run` n'écrit rien.
fn extras_phase(
    env: &Env,
    repo: &Path,
    o: &Opts,
    changed_modules: &std::collections::HashSet<String>,
) -> Result<Extras> {
    let manifest_file = env.manifest_file();
    let mut manifest = Manifest::load(&manifest_file)?;
    let layout = config::load(repo, env)?;
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
        println!("\nSimulation (--dry-run) : orphelins et lignes non touchés.");
        return Ok(Extras {
            conflicts,
            changed: false,
        });
    }
    if !o.yes && !confirm(&format!("\nAppliquer ces {writes} changement(s) ? [o/N] "))? {
        println!("Annulé.");
        return Ok(Extras {
            conflicts,
            changed: false,
        });
    }
    let stamp = time::now_compact();
    // Unités à retirer d'abord : leurs fichiers vont disparaître avec le module.
    units::apply_removals(env, &runner, &unit_steps, &mut manifest)?;
    manifest.save(&manifest_file)?;
    orphans::apply(env, repo, &orphan_list, &stamp, &mut manifest)?;
    manifest.save(&manifest_file)?;
    // Les liens retirés, on replanifie les lignes (un fichier lié disparu se recrée).
    let steps = lines::plan(&specs, &manifest, &std::collections::HashSet::new());
    let n = lines::apply(env, &steps, &stamp, &mut manifest)?;
    units::apply_additions(&runner, &unit_steps, &mut manifest)?;
    manifest.save(&manifest_file)?;
    println!(
        "Terminé : {} orphelin(s) retiré(s), {n} ligne(s) traitée(s).",
        orphan_list.len()
    );
    Ok(Extras {
        conflicts: lines::conflicts(&steps) + units::conflicts(&unit_steps),
        changed: true,
    })
}

/// Rappelle les modules que la config de la machine ne veut pas installer.
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
            "Modules désactivés sur cette machine (config.toml, disabled) : {}",
            off.join(", ")
        );
    }
    Ok(())
}

fn install_phase(env: &Env, repo: &Path, o: &Opts) -> Result<Phase> {
    let (yes, dry_run) = (o.yes, o.dry_run);
    let decisions = decisions(env, o)?;
    if o.no_config && !decisions.is_empty() {
        bail!("--take-repo/--take-home n'ont pas de sens avec --no-config");
    }
    if !o.skip_build {
        build(repo)?;
    }

    let manifest_file = env.manifest_file();
    let mut manifest = Manifest::load(&manifest_file)?;
    let items = plan::desired(repo, env)?;
    for it in &items {
        if !it.src.is_file() {
            bail!("binaire ou fichier source absent : {}", it.src.display());
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

    // Refus net : on n'écrit rien du tout si un fichier étranger gêne un binaire.
    let conflicts: Vec<_> = steps
        .iter()
        .filter(|(_, a)| *a == Action::Conflict)
        .collect();
    if !conflicts.is_empty() {
        eprintln!();
        for (it, _) in &conflicts {
            eprintln!(
                "poste sys : refus : {} existe et n'a pas été posé par poste sys. Déplacez-le ou supprimez-le vous-même, puis relancez.",
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

    // La config : un plan à part, dont les conflits n'empêchent pas le reste.
    let layout = if o.no_config {
        None
    } else {
        config::load(repo, env)?
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
            println!(
                "\n{msg} ({cfg_conflicts} conflit(s) de config à trancher, code de sortie 2)."
            );
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
        end("Tout est à jour");
        return Ok(Phase {
            code,
            proceed: true,
            config_changed: false,
            changed_modules: changed_modules.clone(),
        });
    }
    if dry_run {
        end("Simulation (--dry-run) : rien n'a été écrit");
        return Ok(Phase {
            code,
            proceed: true,
            config_changed: false,
            changed_modules: changed_modules.clone(),
        });
    }
    if !yes
        && !confirm(&format!(
            "\nAppliquer ces {} changement(s) ? [o/N] ",
            to_write + cfg_writes
        ))?
    {
        println!("Annulé.");
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
                    println!("  sauvegarde : {}", env.display(&bak));
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
        "Terminé : {to_write} fichier(s) posé(s), {done_cfg} élément(s) de config traité(s)"
    ));
    Ok(Phase {
        code,
        proceed: true,
        config_changed: done_cfg > 0,
        changed_modules: changed_modules.clone(),
    })
}

fn build(repo: &Path) -> Result<()> {
    println!("Construction : cargo build --release ({})", repo.display());
    let status = Command::new("cargo")
        .args(["build", "--release", "--quiet"])
        .current_dir(repo)
        .status()
        .context("impossible de lancer cargo")?;
    if !status.success() {
        bail!("la construction a échoué");
    }
    Ok(())
}

fn print_plan(env: &Env, steps: &[(Item, Action)]) {
    println!("\nPlan :");
    for (it, a) in steps {
        let note = match a {
            Action::Update { by_hand: true } => {
                "  (écrase une retouche manuelle, ancien fichier sauvegardé)"
            }
            Action::Adopt => "  (déjà identique, désormais suivi)",
            Action::Conflict => "  (fichier étranger, non posé par poste sys)",
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
        Action::Create => "créé",
        Action::Update { .. } => "modifié",
        Action::Unchanged | Action::Adopt => "inchangé",
        Action::Conflict => "REFUSÉ",
    }
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_lowercase().as_str(),
        "o" | "oui" | "y" | "yes"
    ))
}

/// Copie atomique : fichier temporaire voisin puis renommage (fonctionne même si
/// la destination est un binaire en cours d'exécution, y compris poste-sys lui-même).
fn install(src: &Path, dest: &Path) -> Result<()> {
    let parent = dest.parent().context("destination sans dossier parent")?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".poste-tmp-{}", std::process::id()));
    std::fs::copy(src, &tmp).with_context(|| format!("copie vers {}", tmp.display()))?;
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

    /// Faux dépôt (avec un « binaire » déjà construit) et HOME temporaire.
    fn fixture() -> (tempfile::TempDir, PathBuf, Env) {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir_all(repo.join("target/release")).unwrap();
        fs::write(repo.join("target/release/poste"), "bin").unwrap();
        fs::create_dir_all(repo.join("config/.config/a")).unwrap();
        fs::write(repo.join("config/.config/a/neuf"), "n").unwrap();
        fs::write(repo.join("config/.config/a/conflit"), "dépôt").unwrap();
        let home = d.path().join("home");
        fs::create_dir_all(home.join(".config/a")).unwrap();
        fs::write(home.join(".config/a/conflit"), "maison").unwrap();
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
    fn dry_run_n_ecrit_rien() {
        let (_d, repo, env) = fixture();
        let c = run(&env, &repo, &opts(true, true)).unwrap();
        assert_eq!(code(c), code(ExitCode::from(2)), "conflit : code non nul");
        assert!(!env.home.join(".local").exists(), "rien dans .local");
        assert!(!env.home.join(".config/a/neuf").exists());
        assert_eq!(
            fs::read_to_string(env.home.join(".config/a/conflit")).unwrap(),
            "maison"
        );
    }

    #[test]
    fn application_conflit_non_bloquant_puis_relance() {
        let (_d, repo, env) = fixture();
        let c = run(&env, &repo, &opts(true, false)).unwrap();
        assert_eq!(code(c), code(ExitCode::from(2)));
        assert!(env.home.join(".config/a/neuf").is_symlink());
        assert!(env.bin_dir().join("poste").is_file());
        assert!(!env.home.join(".config/a/conflit").is_symlink());
        let m = Manifest::load(&env.manifest_file()).unwrap();
        assert!(m
            .files
            .iter()
            .any(|e| e.module == "config" && e.path.ends_with(".config/a/neuf")));
        assert!(m.files.iter().any(|e| e.module == "poste"));
        // Trancher le conflit puis relancer : à jour, code 0, manifeste inchangé.
        let o = Opts {
            take_repo: vec![".config/a/conflit".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        assert!(env.home.join(".config/a/conflit").is_symlink());
        let before = fs::read_to_string(env.manifest_file()).unwrap();
        assert_eq!(
            code(run(&env, &repo, &opts(true, false)).unwrap()),
            code(ExitCode::SUCCESS)
        );
        assert_eq!(fs::read_to_string(env.manifest_file()).unwrap(), before);
        // L'ancienne version du conflit est sauvegardée.
        let b: Vec<_> = fs::read_dir(env.backup_dir()).unwrap().flatten().collect();
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn sans_config_et_decisions_incoherentes() {
        let (_d, repo, env) = fixture();
        let o = Opts {
            no_config: true,
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        assert!(!env.home.join(".config/a/neuf").exists());
        let o = Opts {
            take_repo: vec!["x".into()],
            take_home: vec!["x".into()],
            ..opts(true, true)
        };
        assert!(run(&env, &repo, &o).is_err());
    }

    #[test]
    fn module_script_lignes_et_disparition() {
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
        fs::write(home.join(".config/x.conf"), "perso = 1\n").unwrap();
        let o = Opts {
            take_repo: vec![".config/a/conflit".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        let prog = env.modules_dir().join("rec/poste-rec");
        assert!(prog.is_file() && env.modules_dir().join("rec/hypr/rec.lua").is_file());
        let conf = fs::read_to_string(home.join(".config/x.conf")).unwrap();
        assert!(conf.starts_with("perso = 1\n") && conf.contains("k:p = "));
        let before = fs::read_to_string(env.manifest_file()).unwrap();
        run(&env, &repo, &opts(true, false)).unwrap();
        assert_eq!(fs::read_to_string(env.manifest_file()).unwrap(), before);
        // Le module disparaît du dépôt : ses fichiers et sa ligne partent, le reste est intact.
        fs::remove_dir_all(&m).unwrap();
        run(&env, &repo, &opts(true, false)).unwrap();
        assert!(!env.modules_dir().join("rec").exists());
        assert_eq!(
            fs::read_to_string(home.join(".config/x.conf")).unwrap(),
            "perso = 1\n"
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
    fn fichiers_locaux_jamais_touches() {
        let (_d, repo, env) = fixture();
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "local = [\".config/a/conflit\"]\n").unwrap();
        // Le fichier différent n'est plus un conflit : code 0, fichier intact, le reste est lié.
        assert_eq!(
            code(run(&env, &repo, &opts(true, false)).unwrap()),
            code(ExitCode::SUCCESS)
        );
        assert!(env.home.join(".config/a/neuf").is_symlink());
        let loc = env.home.join(".config/a/conflit");
        assert!(!loc.is_symlink());
        assert_eq!(fs::read_to_string(&loc).unwrap(), "maison");
        let m = Manifest::load(&env.manifest_file()).unwrap();
        assert!(m.files.iter().all(|e| !e.path.ends_with("a/conflit")));
        // Le diff d'un chemin local montre quand même l'écart.
        let l = config::load(&repo, &env).unwrap().unwrap();
        assert_eq!(l.local.len(), 1);
        assert!(config::diff(&l, &env, Some(".config/a/conflit")).unwrap());
        // Sans diagnostic d'écart sans argument pour les locaux.
        assert!(!config::diff(&l, &env, None).unwrap());
    }

    #[test]
    fn fichier_local_deja_lie_reste_tel_quel() {
        let (_d, repo, env) = fixture();
        run(
            &env,
            &repo,
            &Opts {
                take_repo: vec![".config/a/conflit".into()],
                ..opts(true, false)
            },
        )
        .unwrap();
        assert!(env.home.join(".config/a/conflit").is_symlink());
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "local = [\".config/a\"]\n").unwrap();
        // Un dossier entier en local : plus rien n'est ni créé ni retiré.
        let before = fs::read_to_string(env.manifest_file()).unwrap();
        run(&env, &repo, &opts(true, false)).unwrap();
        assert!(env.home.join(".config/a/conflit").is_symlink());
        assert_eq!(fs::read_to_string(env.manifest_file()).unwrap(), before);
    }

    #[test]
    fn module_desactive_retire_puis_reactive() {
        let (_d, repo, env) = fixture();
        let (fake, log) = crate::units::tests::fake_systemctl(_d.path(), &env.home);
        std::env::set_var("POSTE_SYS_TEST_SYSTEMCTL", &fake);
        module_rec(&repo);
        let home = env.home.clone();
        fs::write(home.join(".config/x.conf"), "perso = 1\n").unwrap();
        let o = Opts {
            take_repo: vec![".config/a/conflit".into()],
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

        // Désactivé : l'aperçu seul ne retire rien.
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "disabled = [\"rec\"]\n").unwrap();
        run(&env, &repo, &opts(true, true)).unwrap();
        assert!(base.join("poste-rec").is_file());
        assert!(home
            .join(".config/systemd/user/poste-rec.service")
            .is_symlink());
        // Pour de vrai : fichiers, hypr/, island/, unité et ligne partent ; le reste est intact.
        run(&env, &repo, &opts(true, false)).unwrap();
        assert!(!base.exists());
        assert!(fs::read_to_string(&log)
            .unwrap()
            .contains("disable --now poste-rec.service"));
        assert!(fs::symlink_metadata(home.join(".config/systemd/user/poste-rec.service")).is_err());
        assert_eq!(
            fs::read_to_string(home.join(".config/x.conf")).unwrap(),
            "perso = 1\n"
        );
        let man = Manifest::load(&env.manifest_file()).unwrap();
        assert!(man.units.is_empty() && man.lines.is_empty());
        assert!(man.files.iter().all(|e| e.module != "rec"));
        assert!(home.join(".config/a/neuf").is_symlink());

        // Réactivé : tout revient.
        fs::write(env.config_file(), "disabled = []\n").unwrap();
        run(&env, &repo, &opts(true, false)).unwrap();
        assert!(base.join("island/launcher.json").is_file());
        assert!(home
            .join(".config/systemd/user/poste-rec.service")
            .is_symlink());
        let man = Manifest::load(&env.manifest_file()).unwrap();
        assert_eq!(man.units.len(), 1);
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
    fn surcharge_json_generee_regeneree_et_conflit() {
        let (_d, repo, env) = fixture();
        let dest = with_override(&repo, &env);
        let o = Opts {
            take_repo: vec![".config/a/conflit".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        // Fichier ordinaire (pas un lien), dépôt + surcharge, reste du fichier suit le dépôt.
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

        // Le dépôt change : régénéré sans conflit, surcharge conservée.
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

        // Modifié à la main : conflit (code 2), fichier intact, status le signale.
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

        // --take-repo régénère (ancien sauvegardé).
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
    fn surcharge_take_home_remet_les_valeurs_du_depot_ou_refuse() {
        let (_d, repo, env) = fixture();
        let dest = with_override(&repo, &env);
        let o = Opts {
            take_repo: vec![".config/a/conflit".into()],
            ..opts(true, false)
        };
        run(&env, &repo, &o).unwrap();
        let t = fs::read_to_string(&dest).unwrap();
        // Réglage fait à la main dans ~ : repris dans le dépôt, sans la valeur surchargée.
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
        // Valeur surchargée changée à la main : reprise ambiguë, refus, rien d'écrit.
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
    fn surcharge_remplace_un_lien_et_adopte_un_fichier_equivalent() {
        let (_d, repo, env) = fixture();
        let dest = with_override(&repo, &env);
        // Ancien lien vers le dépôt (avant la surcharge) : remplacé par un fichier généré.
        std::os::unix::fs::symlink(repo.join("config/.config/a/s.json"), &dest).unwrap();
        let o = Opts {
            take_repo: vec![".config/a/conflit".into()],
            ..opts(true, false)
        };
        assert_eq!(code(run(&env, &repo, &o).unwrap()), code(ExitCode::SUCCESS));
        assert!(!dest.is_symlink());
        // Le PC pro : mêmes valeurs écrites à la main, autre mise en forme : adopté sans conflit.
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
        // Dry-run : rien n'est écrit.
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
