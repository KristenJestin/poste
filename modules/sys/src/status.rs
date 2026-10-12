//! `poste sys status` : dépôt, modules installés, fichiers posés.

use crate::manifest::Manifest;
use crate::plan::{self, Item};
use crate::util::{sha256_file, short};
use crate::{config, lines, orphans};
use anyhow::Result;
use poste_core::Env;
use std::path::Path;
use std::process::Command;

/// Lignes posées par les modules dans des fichiers sans inclusion, et orphelins.
fn print_lines_and_orphans(
    env: &Env,
    repo: &Path,
    layout: Option<&config::Layout>,
    manifest: &Manifest,
    items: &[Item],
) -> Result<()> {
    let orphan_list = orphans::find(repo, env, layout, manifest, items)?;
    let vanishing = orphan_list.iter().map(|o| o.path.clone()).collect();
    let specs = lines::desired(repo, env)?;
    let steps = lines::plan(&specs, manifest, &vanishing);
    let ok = steps
        .iter()
        .filter(|s| matches!(s.op, lines::Op::Unchanged))
        .count();
    println!("\nLignes de module");
    println!("  en place        {ok}");
    println!("  conflits        {}", lines::conflicts(&steps));
    println!(
        "  à poser/retirer {}",
        steps.iter().filter(|s| s.writes()).count()
    );
    for s in steps
        .iter()
        .filter(|s| !matches!(s.op, lines::Op::Unchanged))
    {
        println!(
            "  {:<12} {}  {}  [{}]",
            lines::label(&s.op),
            env.display(&s.file),
            s.line,
            s.module
        );
        if let lines::Op::Conflict(why) | lines::Op::Blocked(why) = &s.op {
            println!("               {why}");
        }
    }
    let runner = crate::units::Runner::from_env();
    let unit_steps = crate::units::plan(
        env,
        &runner,
        &crate::units::desired(repo, env)?,
        manifest,
        &std::collections::HashSet::new(),
    );
    if !unit_steps.is_empty() {
        println!("\nUnités systemd utilisateur");
        for s in &unit_steps {
            println!(
                "  {:<13} {}  [{}]",
                crate::units::label(&s.op),
                s.unit,
                s.module
            );
            if let crate::units::Op::Conflict(why) = &s.op {
                println!("                {why}");
            }
        }
    }
    println!("\nOrphelins");
    println!("  à retirer       {}", orphan_list.len());
    if !orphan_list.is_empty() {
        orphans::print(env, &orphan_list);
        println!("  (poste sys setup propose de les retirer)");
    }
    Ok(())
}

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
    repo_section("Dépôt", repo);
    if let Some(d) = poste_core::Config::load(env)?.data_path(env) {
        println!();
        if d.is_dir() {
            repo_section("Dépôt de données de la machine (data)", &d);
        } else {
            println!("Dépôt de données de la machine (data)");
            println!("  chemin        {} (absent)", d.display());
        }
    }
    rest(env, repo)
}

/// Chemin, branche, modifications et poussée d'un dépôt git.
fn repo_section(title: &str, repo: &Path) {
    println!("{title}");
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
}

fn rest(env: &Env, repo: &Path) -> Result<()> {
    let manifest = Manifest::load(&env.manifest_file())?;
    let items = plan::desired(repo, env)?;

    let cfg = poste_core::Config::load(env)?;
    if !cfg.disabled.is_empty() {
        println!(
            "\nModules désactivés sur cette machine (config.toml, disabled) : {}",
            cfg.disabled.join(", ")
        );
    }
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
    let layout = config::load(repo, env)?;
    if let Some(layout) = &layout {
        config::print_status(layout, env, &manifest)?;
    }
    print_lines_and_orphans(env, repo, layout.as_ref(), &manifest, &items)?;
    crate::provision::status(env, repo)?;
    Ok(())
}
