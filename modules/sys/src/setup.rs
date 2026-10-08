//! `poste sys setup` : construit, montre le plan, demande confirmation, installe.

use crate::manifest::{Entry, Manifest};
use crate::plan::{self, Action, Item};
use crate::util::sha256_file;
use anyhow::{bail, Context, Result};
use poste_core::{time, Env};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

pub fn run(env: &Env, repo: &Path, yes: bool, dry_run: bool) -> Result<ExitCode> {
    build(repo)?;

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

    // Refus net : on n'écrit rien du tout si un fichier étranger gêne.
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
        return Ok(ExitCode::from(1));
    }

    let to_write = steps
        .iter()
        .filter(|(_, a)| matches!(a, Action::Create | Action::Update { .. }))
        .count();
    let to_adopt = steps.iter().filter(|(_, a)| *a == Action::Adopt).count();

    if to_write == 0 {
        println!("\nTout est à jour.");
        if to_adopt > 0 && !dry_run {
            record_adoptions(&steps, &mut manifest, &manifest_file)?;
        }
        return Ok(ExitCode::SUCCESS);
    }
    if dry_run {
        println!("\nSimulation (--dry-run) : rien n'a été écrit.");
        return Ok(ExitCode::SUCCESS);
    }
    if !yes
        && !confirm(&format!(
            "\nAppliquer ces {to_write} changement(s) ? [o/N] "
        ))?
    {
        println!("Annulé.");
        return Ok(ExitCode::from(1));
    }

    let stamp = time::now_compact();
    for (it, action) in &steps {
        match action {
            Action::Create | Action::Update { .. } => {
                if matches!(action, Action::Update { .. }) {
                    let bak = backup(env, &it.dest, &stamp)?;
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
    println!("\nTerminé : {to_write} fichier(s) posé(s).");
    Ok(ExitCode::SUCCESS)
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

/// Copie l'ancien fichier dans `backup/` sous un nom horodaté ; renvoie ce chemin.
fn backup(env: &Env, dest: &Path, stamp: &str) -> Result<PathBuf> {
    let dir = env.backup_dir();
    std::fs::create_dir_all(&dir)?;
    let rel = dest.strip_prefix(&env.home).unwrap_or(dest);
    let flat: String = rel
        .to_string_lossy()
        .trim_start_matches('/')
        .chars()
        .map(|c| if c == '/' { '_' } else { c })
        .collect();
    let bak = dir.join(format!("{flat}.{stamp}.bak"));
    std::fs::copy(dest, &bak).with_context(|| format!("sauvegarde de {}", dest.display()))?;
    Ok(bak)
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

fn record_adoptions(steps: &[(Item, Action)], manifest: &mut Manifest, file: &Path) -> Result<()> {
    for (it, a) in steps {
        if *a == Action::Adopt {
            record(manifest, it)?;
        }
    }
    manifest.save(file)
}
