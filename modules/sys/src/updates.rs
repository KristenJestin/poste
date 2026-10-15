//! `poste sys outdated` and `poste sys update`: what is behind its source, and the update of
//! the apps (`[[app]]`) and third-party plugins. The packages of the repositories (pacman,
//! Omarchy, AUR) are only reported: their update stays `omarchy update` or `yay -Syu`.

use crate::apps;
use crate::provision::{self, Ctx};
use anyhow::{bail, Result};
use poste_core::Env;
use std::path::Path;
use std::process::{Command, Stdio};

/// Where one app or plugin stands.
enum Item {
    App {
        app: apps::App,
        installed: Option<String>,
        tag: String,
    },
    Plugin {
        id: String,
        dir: std::path::PathBuf,
        patch: Option<std::path::PathBuf>,
        at: String,
        head: String,
        name: String,
        commit: String,
        ahead: bool,
    },
}

impl Item {
    fn id(&self) -> &str {
        match self {
            Item::App { app, .. } => &app.id,
            Item::Plugin { id, .. } => id,
        }
    }

    fn behind(&self) -> bool {
        match self {
            Item::App { installed, tag, .. } => installed.as_deref() != Some(&apps::norm(tag)),
            Item::Plugin {
                head,
                commit,
                ahead,
                ..
            } => head != commit && !ahead,
        }
    }

    fn line(&self) -> String {
        match self {
            Item::App { installed, tag, .. } => {
                let now = installed.as_deref().unwrap_or("not installed");
                if self.behind() {
                    format!("{now} -> {}", apps::norm(tag))
                } else {
                    format!("{now} (up to date)")
                }
            }
            Item::Plugin {
                at,
                name,
                commit,
                ahead,
                ..
            } => {
                if self.behind() {
                    format!("{at} -> {name} ({})", &commit[..commit.len().min(7)])
                } else if *ahead {
                    format!("{at} (ahead of {name}: nothing to do)")
                } else {
                    format!("{at} (up to date, {name})")
                }
            }
        }
    }
}

/// Reads where each app and plugin is and where its source is (network, nothing written).
/// `only`: these identifiers (all if empty). Errors are printed and the item left out.
fn survey(ctx: &Ctx, steps: &provision::Steps, only: &[String]) -> Vec<Item> {
    let mut out = survey_apps(ctx, steps, only);
    out.extend(survey_plugins(ctx, steps, only));
    out
}

fn wanted(only: &[String], id: &str) -> bool {
    only.is_empty() || only.iter().any(|o| o == id)
}

fn survey_apps(ctx: &Ctx, steps: &provision::Steps, only: &[String]) -> Vec<Item> {
    let mut out = Vec::new();
    for a in steps.app.iter().filter(|a| wanted(only, &a.id)) {
        match apps::latest_tag(a, &ctx.tools) {
            Ok(tag) => out.push(Item::App {
                installed: apps::installed(a, &ctx.sub_cmd(&a.version), &ctx.tools),
                app: a.clone(),
                tag,
            }),
            Err(e) => println!("  {:<14} check impossible: {e:#}", a.id),
        }
    }
    out
}

fn survey_plugins(ctx: &Ctx, steps: &provision::Steps, only: &[String]) -> Vec<Item> {
    let mut out = Vec::new();
    for p in steps.plugin.iter().filter(|p| wanted(only, &p.id)) {
        let dir = ctx.plugin_dir(&p.id);
        if !dir.join(".git").exists() {
            println!(
                "  {:<14} not installed as a git clone (poste sys setup)",
                p.id
            );
            continue;
        }
        match apps::plugin_state(&dir, &ctx.tools) {
            Ok(apps::PluginState {
                head,
                target: Some((name, commit)),
                ahead,
            }) => out.push(Item::Plugin {
                id: p.id.clone(),
                at: apps::describe(&dir, &ctx.tools),
                patch: p.patch.as_ref().map(|r| ctx.patch_path(r)),
                dir,
                head,
                name,
                commit,
                ahead,
            }),
            Ok(_) => println!("  {:<14} no tag nor default branch upstream", p.id),
            Err(e) => println!("  {:<14} check impossible: {e:#}", p.id),
        }
    }
    out
}

fn width(items: &[Item]) -> usize {
    items.iter().map(|i| i.id().len()).max().unwrap_or(0)
}

fn print_items(items: &[Item]) {
    let w = width(items);
    for i in items {
        println!("  {:<w$} {}", i.id(), i.line());
    }
}

/// Lines of a command that lists updates (`name old -> new`); `None` if it failed.
/// `none`: its exit code when there is nothing to update (`checkupdates` 2, `yay -Qua` 1).
fn listing(prog: &str, args: &[&str], none: i32) -> Option<Vec<String>> {
    let o = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let lines: Vec<String> = String::from_utf8_lossy(&o.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    match o.status.code() {
        Some(0) => Some(lines),
        Some(c) if c == none && lines.is_empty() => Some(lines),
        _ => None,
    }
}

/// `poste sys outdated`: apps, plugins, then the system packages.
pub fn outdated(env: &Env, repo: &Path) -> Result<()> {
    let ctx = Ctx::new(env, repo);
    let (_, steps, _) = provision::load_wanted(&ctx)?;
    println!("Apps (GitHub releases)");
    let items = survey_apps(&ctx, &steps, &[]);
    print_items(&items);
    println!("\nOmarchy plugins");
    let plugins = survey_plugins(&ctx, &steps, &[]);
    print_items(&plugins);
    let behind = items.iter().chain(&plugins).filter(|i| i.behind()).count();
    if behind > 0 {
        println!("\n  {behind} to update: poste sys update (or poste sys update <id>)");
    }

    println!("\nPackages (pacman repositories, Omarchy included, then AUR)");
    let repos = listing("checkupdates", &["--nocolor"], 2);
    let aur = listing("yay", &["-Qua"], 1);
    for (what, l) in [("repositories", &repos), ("AUR", &aur)] {
        match l {
            None => println!("  {what}: check impossible"),
            Some(l) if l.is_empty() => println!("  {what}: up to date"),
            Some(l) => {
                println!("  {what}: {} to update", l.len());
                for x in l {
                    println!("    {x}");
                }
            }
        }
    }
    let any = |l: &Option<Vec<String>>| l.as_ref().is_some_and(|l| !l.is_empty());
    if any(&repos) || any(&aur) {
        let omarchy = repos
            .iter()
            .flatten()
            .any(|l| l.split_whitespace().next() == Some("omarchy"));
        println!("  To run yourself (poste does not run it):");
        if omarchy || provision::have(env, "omarchy-update") {
            println!("    omarchy update   (packages, AUR and Omarchy's migrations)");
        } else {
            println!("    yay -Syu");
        }
    }
    Ok(())
}

/// `poste sys update [<id>…]`: moves the apps and plugins that are behind (all, or the cited
/// ones) to their latest version, after a preview (the new commits of a plugin) and a
/// confirmation. Returns the exit code: 0, or 1 if one failed.
pub fn update(
    env: &Env,
    repo: &Path,
    ids: &[String],
    yes: bool,
    dry_run: bool,
    confirm: &dyn Fn(&str) -> Result<bool>,
) -> Result<u8> {
    let ctx = Ctx::new(env, repo);
    let (_, steps, _) = provision::load_wanted(&ctx)?;
    let known: Vec<&str> = steps
        .app
        .iter()
        .map(|a| a.id.as_str())
        .chain(steps.plugin.iter().map(|p| p.id.as_str()))
        .collect();
    if let Some(bad) = ids.iter().find(|i| !known.contains(&i.as_str())) {
        bail!(
            "{bad}: neither an app nor a plugin of this machine. Known: {}",
            known.join(", ")
        );
    }
    let items = survey(&ctx, &steps, ids);
    let (todo, fresh): (Vec<Item>, Vec<Item>) = items.into_iter().partition(|i| i.behind());
    print_items(&fresh);
    if todo.is_empty() {
        println!("Nothing to update.");
        return Ok(0);
    }
    println!("To update");
    let w = width(&todo);
    for i in &todo {
        println!("  {:<w$} {}", i.id(), i.line());
        if let Item::Plugin { dir, commit, .. } = i {
            match apps::plugin_log(dir, commit, !dry_run, &ctx.tools) {
                Ok(log) if log.is_empty() => {}
                Ok(log) => {
                    let lines: Vec<&str> = log.lines().collect();
                    for l in lines.iter().take(30) {
                        println!("      {l}");
                    }
                    if lines.len() > 30 {
                        println!("      … {} more commits", lines.len() - 30);
                    }
                }
                Err(_) if dry_run => println!("      new commits not fetched (dry run)"),
                Err(e) => println!("      new commits unreadable: {e:#}"),
            }
        }
    }
    if dry_run {
        println!("\nDry run (--dry-run): nothing updated");
        return Ok(0);
    }
    if !yes && !confirm(&format!("\nUpdate these {}? [y/N] ", todo.len()))? {
        println!("Nothing updated.");
        return Ok(0);
    }
    let mut done = Vec::new();
    let mut failed = Vec::new();
    let mut by_hand = Vec::new();
    for i in &todo {
        println!("\n  {}", i.id());
        let res = match i {
            Item::App { app, tag, .. } => apps::install(
                app,
                Some(tag),
                &ctx.sub_cmd(&app.busy),
                &env.bin_dir(),
                &env.cache_dir(),
                &ctx.tools,
            )
            .map(|r| match r.by_hand {
                // Not done until the user runs it: triggers nothing.
                Some(c) => {
                    by_hand.push(c);
                    false
                }
                None => true,
            }),
            Item::Plugin {
                dir, commit, patch, ..
            } => apps::plugin_move(dir, commit, patch.as_deref(), &ctx.tools).map(|()| true),
        };
        match res {
            Ok(true) => {
                println!("    done");
                done.push(i.id().to_string());
            }
            Ok(false) => println!("    downloaded and checked, the sudo command is below"),
            Err(e) => {
                eprintln!("    failure: {e:#}");
                failed.push(i.id().to_string());
            }
        }
    }
    if !done.is_empty() {
        match provision::run_triggered(&ctx, &steps, &done) {
            Ok(f) => failed.extend(f),
            Err(e) => {
                eprintln!("\nsteps triggered by the update not run: {e:#}");
                failed.push("triggered steps".into());
            }
        }
    }
    if !by_hand.is_empty() {
        println!("\nTo run yourself (sudo, poste never does it):");
        for c in &by_hand {
            println!("  {c}");
        }
    }
    if todo
        .iter()
        .any(|i| matches!(i, Item::Plugin { .. }) && done.iter().any(|d| d == i.id()))
    {
        println!(
            "\nPlugins updated: `omarchy restart shell` loads them (check `poste record status` first)."
        );
    }
    if !failed.is_empty() {
        eprintln!("\nfailed: {}", failed.join(", "));
        return Ok(1);
    }
    Ok(0)
}
