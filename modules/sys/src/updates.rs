//! `poste sys outdated` and `poste sys update`: what is behind its source, and the update of
//! everything at once: the apps (`[[app]]`) and third-party plugins by poste itself, then the
//! downloaded packages with `sudo pacman -U`, run in the terminal after the confirmation: the
//! user types the password. The system packages (repositories, AUR, Omarchy) are not poste's:
//! never listed nor updated here.

use crate::apps::{self, Standing};
use crate::provision::{self, Ctx};
use anyhow::{bail, Result};
use poste_core::Env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where one app or plugin stands.
enum Item {
    App {
        app: apps::App,
        installed: Option<String>,
        target: apps::Target,
        standing: Standing,
    },
    Plugin {
        id: String,
        dir: PathBuf,
        patch: Option<apps::Patch>,
        /// Carries an earlier version of its patch.
        stale: bool,
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

    /// A plugin whose commit changes (never moved back).
    fn moves(&self) -> bool {
        matches!(self, Item::Plugin { head, commit, ahead, .. } if head != commit && !ahead)
    }

    fn behind(&self) -> bool {
        match self {
            Item::App { standing, .. } => {
                matches!(standing, Standing::Behind | Standing::Missing)
            }
            Item::Plugin { stale, .. } => self.moves() || *stale,
        }
    }

    fn line(&self) -> String {
        match self {
            Item::App {
                installed,
                target,
                standing,
                ..
            } => {
                let now = installed.as_deref().unwrap_or("not installed");
                let to = &target.version;
                match standing {
                    Standing::Missing | Standing::Behind => format!("{now} -> {to}"),
                    Standing::UpToDate => format!("{now} (up to date)"),
                    Standing::Ahead => format!("{now} (newer than {to}: nothing to do)"),
                    Standing::Unknown => format!(
                        "{now} (not a published release, cannot be compared with {to}: left alone)"
                    ),
                }
            }
            Item::Plugin {
                at,
                name,
                commit,
                ahead,
                stale,
                patch,
                ..
            } => {
                let removed = patch.as_ref().is_some_and(|p| p.file.is_none());
                let renew = match (stale, removed) {
                    (false, _) => "",
                    (true, false) => ", new local patch",
                    (true, true) => ", local patch removed",
                };
                if self.moves() {
                    format!("{at} -> {name} ({}){renew}", &commit[..commit.len().min(7)])
                } else if *stale {
                    format!(
                        "{at} (local patch to {})",
                        if removed { "remove" } else { "renew" }
                    )
                } else if *ahead {
                    format!("{at} (ahead of {name}: nothing to do)")
                } else {
                    format!("{at} (up to date, {name})")
                }
            }
        }
    }
}

fn wanted(only: &[String], id: &str) -> bool {
    only.is_empty() || only.iter().any(|o| o == id)
}

/// Reads where each app is and where its source is (network, nothing written). Errors are
/// printed and the app left out.
fn survey_apps(ctx: &Ctx, steps: &provision::Steps, only: &[String]) -> Vec<Item> {
    let mut out = Vec::new();
    for a in steps.app.iter().filter(|a| wanted(only, &a.id)) {
        let found = apps::releases(a, &ctx.tools).and_then(|rel| {
            let t = apps::target(a, &rel, &ctx.tools)?;
            Ok((rel, t))
        });
        match found {
            Ok((rel, target)) => {
                let installed = apps::installed(a, &ctx.sub_cmd(&a.version), &ctx.tools);
                out.push(Item::App {
                    standing: apps::standing(installed.as_deref(), &target, &rel),
                    installed,
                    target,
                    app: a.clone(),
                })
            }
            Err(e) => println!("  {:<14} check impossible: {e:#}", a.id),
        }
    }
    out
}

/// The same for the plugins (`git ls-remote`, nothing written).
fn survey_plugins(ctx: &Ctx, steps: &provision::Steps, only: &[String]) -> Vec<Item> {
    let mut out = Vec::new();
    for p in steps.plugin.iter().filter(|p| wanted(only, &p.id)) {
        let dir = ctx.plugin_dir(&p.id);
        let patch = ctx.plugin_patch(p);
        if !dir.join(".git").exists() {
            println!(
                "  {:<14} not installed as a git clone (poste sys setup)",
                p.id
            );
            continue;
        }
        match apps::plugin_state(&dir, p.prerelease, &ctx.tools) {
            Ok(apps::PluginState {
                head,
                target: Some((name, commit)),
                ahead,
            }) => out.push(Item::Plugin {
                id: p.id.clone(),
                at: apps::describe(&dir, &ctx.tools),
                stale: patch
                    .as_ref()
                    .is_some_and(|pa| pa.stale(pa.on(&dir, &ctx.tools).as_ref())),
                patch,
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

/// `poste sys outdated`: apps and plugins. The system packages are not poste's: never listed
/// nor updated here.
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
        println!("\nTo update everything: poste sys update (or poste sys update <id>)");
    }
    Ok(())
}

/// Runs a command in the terminal (its sudo asks for the password); true if it succeeded.
fn run_here(cmd: &[String]) -> bool {
    println!("\n$ {}", cmd.join(" "));
    let Some((p, args)) = cmd.split_first() else {
        return false;
    };
    match Command::new(p).args(args).status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            eprintln!("  {} failed ({s})", cmd.join(" "));
            false
        }
        Err(e) => {
            eprintln!("  cannot run {p}: {e}");
            false
        }
    }
}

/// `poste sys update [<id>…]`: everything at once, or only the cited apps and plugins. Moves the
/// apps and plugins that are behind to their latest version, then, with sudo in the terminal,
/// installs the downloaded packages, all after one preview (the new commits of a plugin) and
/// one confirmation. Returns the exit code: 0, or 1 if one failed.
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
    let mut items = survey_apps(&ctx, &steps, ids);
    items.extend(survey_plugins(&ctx, &steps, ids));
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
        match i {
            Item::Plugin { dir, commit, .. } if i.moves() => {
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
            Item::App { app, .. } if app.package.is_some() => {
                println!("      downloaded and checked, then sudo pacman -U (your password)")
            }
            _ => {}
        }
    }
    if dry_run {
        println!("\nDry run (--dry-run): nothing updated");
        return Ok(0);
    }
    if !yes && !confirm("\nUpdate all of this? [y/N] ")? {
        println!("Nothing updated.");
        return Ok(0);
    }
    let mut done = Vec::new();
    let mut failed = Vec::new();
    let mut with_sudo: Vec<(String, Vec<String>)> = Vec::new();
    for i in &todo {
        println!("\n  {}", i.id());
        let res = match i {
            Item::App { app, target, .. } => apps::install(
                app,
                Some(target),
                &ctx.sub_cmd(&app.busy),
                &env.bin_dir(),
                &env.cache_dir(),
                &ctx.tools,
            )
            .map(|r| match r.by_hand {
                // Done once pacman has installed it, below.
                Some(c) => {
                    with_sudo.push((i.id().to_string(), c));
                    false
                }
                None => true,
            }),
            Item::Plugin {
                dir,
                head,
                commit,
                patch,
                ahead,
                ..
            } => {
                let to = apps::destination(head, commit, *ahead);
                apps::plugin_move(dir, to, patch.as_ref(), &ctx.tools).map(|()| true)
            }
        };
        match res {
            Ok(true) => {
                println!("    done");
                done.push(i.id().to_string());
            }
            Ok(false) => println!("    downloaded and checked"),
            Err(e) => {
                eprintln!("    failure: {e:#}");
                failed.push(i.id().to_string());
            }
        }
    }
    for (id, cmd) in &with_sudo {
        if run_here(cmd) {
            done.push(id.clone());
        } else {
            failed.push(id.clone());
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
