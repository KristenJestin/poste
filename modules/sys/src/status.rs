//! `poste sys status`: repository, installed modules, files put in place.

use crate::manifest::Manifest;
use crate::plan::{self, Item};
use crate::util::{sha256_file, short};
use crate::{config, lines, orphans};
use anyhow::Result;
use poste_core::Env;
use std::path::Path;
use std::process::Command;

/// Lines put by modules in files without includes, and orphans.
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
    println!("\nModule lines");
    println!("  in place        {ok}");
    println!("  conflicts       {}", lines::conflicts(&steps));
    println!(
        "  to add/remove   {}",
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
        println!("\nSystemd user units");
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
    println!("\nOrphans");
    println!("  to remove       {}", orphan_list.len());
    if !orphan_list.is_empty() {
        orphans::print(env, &orphan_list);
        println!("  (poste sys setup offers to remove them)");
    }
    Ok(())
}

/// Text output of a git command, `None` if it fails.
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

pub fn run(env: &Env, cfg: &poste_core::Config, repo: &Path) -> Result<()> {
    println!("Source");
    println!("  mode          {}", cfg.source.as_str());
    println!("  root          {}", crate::sync::describe_root(env, repo));
    if !env.current_link().exists() {
        println!(
            "  current       absent (the next `poste sys setup` puts {})",
            env.display(&env.current_link())
        );
    }
    println!(
        "  versions      {} installed (poste sys versions)",
        crate::versions::installed(env).len()
    );
    if let Some(clone) = cfg.repo_path(env).filter(|c| c.is_dir()) {
        println!();
        repo_section("Repository", &clone);
    }
    if let Some(d) = cfg.data_path(env) {
        println!();
        if d.is_dir() {
            repo_section("Machine data repository (data)", &d);
        } else {
            println!("Machine data repository (data)");
            println!("  path          {} (absent)", d.display());
        }
    }
    rest(env, repo)
}

/// Path, branch, changes and push state of a git repository.
fn repo_section(title: &str, repo: &Path) {
    println!("{title}");
    println!("  path          {}", repo.display());
    if git(repo, &["rev-parse", "--git-dir"]).is_none() {
        println!("  git           this folder is not a git repository");
    } else {
        let branch = git(repo, &["branch", "--show-current"]).filter(|b| !b.is_empty());
        println!(
            "  branch        {}",
            branch
                .as_deref()
                .unwrap_or("(none, detached HEAD or no commit)")
        );
        let dirty = git(repo, &["status", "--porcelain"])
            .map(|s| s.lines().count())
            .unwrap_or(0);
        if dirty == 0 {
            println!("  changes       none");
        } else {
            println!("  changes       {dirty} uncommitted file(s)");
        }
        let remotes = git(repo, &["remote"]).unwrap_or_default();
        if remotes.is_empty() {
            println!("  push          no remote");
        } else {
            match git(repo, &["rev-list", "--count", "@{upstream}..HEAD"]) {
                Some(n) if n == "0" => println!("  push          nothing to push"),
                Some(n) => println!("  push          {n} unpushed commit(s)"),
                None => println!("  push          no upstream branch configured"),
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
            "\nModules disabled on this machine (config.toml, disabled): {}",
            cfg.disabled.join(", ")
        );
    }
    println!("\nModules (installed / repository)");
    // Group by module keeping the order: poste first, then the modules.
    let mut names: Vec<&str> = Vec::new();
    for it in &items {
        if !names.contains(&it.module.as_str()) {
            names.push(&it.module);
        }
    }
    for name in names {
        let group: Vec<&Item> = items.iter().filter(|i| i.module == name).collect();
        // The first element of the group is always the binary.
        let bin = group[0];
        let repo_hash = sha256_file(&bin.src).ok();
        let inst_hash = sha256_file(&bin.dest).ok();
        let version = format!(
            "{} / {}",
            inst_hash.as_deref().map(short).unwrap_or("-"),
            repo_hash.as_deref().map(short).unwrap_or("not built")
        );
        let verdict = if inst_hash.is_none() {
            "not installed".to_string()
        } else if repo_hash.is_none() {
            "repository binary not built".to_string()
        } else {
            let outdated = group
                .iter()
                .filter(|i| match (sha256_file(&i.src), sha256_file(&i.dest)) {
                    (Ok(s), Ok(d)) => s != d,
                    _ => true,
                })
                .count();
            if outdated == 0 {
                "up to date".to_string()
            } else {
                format!("to install: {outdated} file(s) differ (poste sys setup)")
            }
        };
        println!("  {name:<8} {version:<20} {verdict}");
    }

    println!("\nFiles put in place");
    let mut problems = 0;
    for e in manifest.files.iter().filter(|e| e.module != config::MODULE) {
        let p = Path::new(&e.path);
        match sha256_file(p) {
            Err(_) => {
                problems += 1;
                println!("  missing           {}  [{}]", env.display(p), e.module);
            }
            Ok(h) if h != e.sha256 => {
                problems += 1;
                println!("  edited by hand    {}  [{}]", env.display(p), e.module);
            }
            Ok(_) => {}
        }
    }
    if problems == 0 {
        println!(
            "  {} tracked file(s), all conform",
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
