//! `poste sys setup` : construit, montre le plan, demande confirmation, installe.

use crate::config::{self, Decision};
use crate::manifest::{Entry, Manifest};
use crate::plan::{self, Action, Item};
use crate::util::sha256_file;
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
}

pub fn run(env: &Env, repo: &Path, o: &Opts) -> Result<ExitCode> {
    let ph = install_phase(env, repo, o)?;
    if !ph.proceed || o.no_config {
        return Ok(ph.code);
    }
    println!("\n== Machine (bootstrap/) ==");
    let applied = crate::provision::setup(env, repo, o.yes, o.dry_run, &confirm)?;
    let mut code = ph.code;
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
    if ph.config_changed && !o.dry_run {
        println!(
            "\nConfig modifiée : pour la recharger, lancez vous-même `omarchy restart shell` et `hyprctl reload`."
        );
    }
    Ok(code)
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
        });
    }
    if dry_run {
        end("Simulation (--dry-run) : rien n'a été écrit");
        return Ok(Phase {
            code,
            proceed: true,
            config_changed: false,
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
}
