//! `poste sys status` : dépôt, modules installés, fichiers posés.

use crate::config;
use crate::manifest::Manifest;
use crate::plan::{self, Item};
use crate::util::{sha256_file, short};
use anyhow::Result;
use poste_core::Env;
use std::path::Path;
use std::process::Command;

/// Sortie texte d'une commande git, `None` si elle échoue.
fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn run(env: &Env, repo: &Path) -> Result<()> {
    println!("Dépôt");
    println!("  chemin        {}", repo.display());
    if git(repo, &["rev-parse", "--git-dir"]).is_none() {
        println!("  git           ce dossier n'est pas un dépôt git");
    } else {
        let branch = git(repo, &["branch", "--show-current"]).filter(|b| !b.is_empty());
        println!(
            "  branche       {}",
            branch
                .as_deref()
                .unwrap_or("(aucune, HEAD détachée ou sans commit)")
        );
        let dirty = git(repo, &["status", "--porcelain"])
            .map(|s| s.lines().count())
            .unwrap_or(0);
        if dirty == 0 {
            println!("  modifications aucune");
        } else {
            println!("  modifications {dirty} fichier(s) non commité(s)");
        }
        let remotes = git(repo, &["remote"]).unwrap_or_default();
        if remotes.is_empty() {
            println!("  poussée       aucun remote");
        } else {
            match git(repo, &["rev-list", "--count", "@{upstream}..HEAD"]) {
                Some(n) if n == "0" => println!("  poussée       rien à pousser"),
                Some(n) => println!("  poussée       {n} commit(s) non poussé(s)"),
                None => println!("  poussée       pas de branche amont configurée"),
            }
        }
    }

    let manifest = Manifest::load(&env.manifest_file())?;
    let items = plan::desired(repo, env)?;

    println!("\nModules (installé / dépôt)");
    // Regroupe par module en gardant l'ordre : poste d'abord, puis les modules.
    let mut names: Vec<&str> = Vec::new();
    for it in &items {
        if !names.contains(&it.module.as_str()) {
            names.push(&it.module);
        }
    }
    for name in names {
        let group: Vec<&Item> = items.iter().filter(|i| i.module == name).collect();
        // Le premier élément du groupe est toujours le binaire.
        let bin = group[0];
        let repo_hash = sha256_file(&bin.src).ok();
        let inst_hash = sha256_file(&bin.dest).ok();
        let version = format!(
            "{} / {}",
            inst_hash.as_deref().map(short).unwrap_or("-"),
            repo_hash.as_deref().map(short).unwrap_or("non construit")
        );
        let verdict = if inst_hash.is_none() {
            "non installé".to_string()
        } else if repo_hash.is_none() {
            "binaire du dépôt non construit".to_string()
        } else {
            let outdated = group
                .iter()
                .filter(|i| match (sha256_file(&i.src), sha256_file(&i.dest)) {
                    (Ok(s), Ok(d)) => s != d,
                    _ => true,
                })
                .count();
            if outdated == 0 {
                "à jour".to_string()
            } else {
                format!("à installer : {outdated} fichier(s) diffèrent (poste sys setup)")
            }
        };
        println!("  {name:<8} {version:<20} {verdict}");
    }

    println!("\nFichiers posés");
    let mut problems = 0;
    for e in manifest.files.iter().filter(|e| e.module != config::MODULE) {
        let p = Path::new(&e.path);
        match sha256_file(p) {
            Err(_) => {
                problems += 1;
                println!("  manquant         {}  [{}]", env.display(p), e.module);
            }
            Ok(h) if h != e.sha256 => {
                problems += 1;
                println!("  modifié à la main {}  [{}]", env.display(p), e.module);
            }
            Ok(_) => {}
        }
    }
    if problems == 0 {
        println!(
            "  {} fichier(s) suivi(s), tous conformes",
            manifest
                .files
                .iter()
                .filter(|e| e.module != config::MODULE)
                .count()
        );
    }
    if let Some(layout) = config::load(repo, env)? {
        config::print_status(&layout, env, &manifest)?;
    }
    Ok(())
}
