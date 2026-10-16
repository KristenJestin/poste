//! `poste sys outdated` and `poste sys update`: what is behind its source, and the update of
//! everything at once: the apps (`[[app]]`) and third-party plugins by poste itself, then the
//! downloaded packages with `sudo pacman -U`, run in the terminal after the confirmation: the
//! user types the password. The system packages (repositories, AUR, Omarchy) are not poste's:
//! never listed nor updated here.

use crate::apps::{self, Standing};
use crate::provision::{self, Ctx};
use crate::reload;
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

/// Where an app is and where its source is (network, nothing written).
fn survey_app(ctx: &Ctx, a: &apps::App) -> Result<Item, String> {
    let (rel, target) = apps::releases(a, &ctx.tools)
        .and_then(|rel| {
            let t = apps::target(a, &rel, &ctx.tools)?;
            Ok((rel, t))
        })
        .map_err(|e| format!("check impossible: {e:#}"))?;
    let installed = apps::installed(a, &ctx.sub_cmd(&a.version), &ctx.tools);
    Ok(Item::App {
        standing: apps::standing(installed.as_deref(), &target, &rel),
        installed,
        target,
        app: a.clone(),
    })
}

/// The same for a plugin (`git ls-remote`, nothing written).
fn survey_plugin(ctx: &Ctx, p: &provision::Plugin) -> Result<Item, String> {
    let dir = ctx.plugin_dir(&p.id);
    let patch = ctx.plugin_patch(p);
    if !dir.join(".git").exists() {
        return Err("not installed as a git clone (poste sys setup)".into());
    }
    match apps::plugin_state(&dir, p.prerelease, &ctx.tools) {
        Ok(apps::PluginState {
            head,
            target: Some((name, commit)),
            ahead,
        }) => Ok(Item::Plugin {
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
        Ok(_) => Err("no tag nor default branch upstream".into()),
        Err(e) => Err(format!("check impossible: {e:#}")),
    }
}

/// Checks the apps and plugins (all, or the cited ones) at the same time, one thread each (a
/// few network calls, not a load), and prints each line as soon as it is known. Errors are
/// printed and the item left out; the items come back in the order of the steps file. The
/// lines of the items behind only with `behind_too` (`update` lists them after).
fn survey(ctx: &Ctx, steps: &provision::Steps, only: &[String], behind_too: bool) -> Vec<Item> {
    let apps: Vec<_> = steps.app.iter().filter(|a| wanted(only, &a.id)).collect();
    let plugins: Vec<_> = steps
        .plugin
        .iter()
        .filter(|p| wanted(only, &p.id))
        .collect();
    let ids: Vec<&str> = apps
        .iter()
        .map(|a| a.id.as_str())
        .chain(plugins.iter().map(|p| p.id.as_str()))
        .collect();
    let w = ids.iter().map(|i| i.len()).max().unwrap_or(0);
    println!("Checking…");
    let (tx, rx) = std::sync::mpsc::channel();
    let found: Vec<Option<Item>> = std::thread::scope(|s| {
        for (n, &a) in apps.iter().enumerate() {
            let tx = tx.clone();
            s.spawn(move || {
                let _ = tx.send((n, survey_app(ctx, a)));
            });
        }
        let first = apps.len();
        for (n, &p) in plugins.iter().enumerate() {
            let tx = tx.clone();
            s.spawn(move || {
                let _ = tx.send((first + n, survey_plugin(ctx, p)));
            });
        }
        drop(tx);
        let mut found: Vec<Option<Item>> = (0..ids.len()).map(|_| None).collect();
        for (n, r) in rx {
            match r {
                Ok(i) => {
                    if behind_too || !i.behind() {
                        println!("  {:<w$} {}", i.id(), i.line());
                    }
                    found[n] = Some(i);
                }
                Err(e) => println!("  {:<w$} {e}", ids[n]),
            }
        }
        found
    });
    found.into_iter().flatten().collect()
}

fn width(items: &[Item]) -> usize {
    items.iter().map(|i| i.id().len()).max().unwrap_or(0)
}

/// `poste sys outdated`: apps and plugins. The system packages are not poste's: never listed
/// nor updated here.
pub fn outdated(env: &Env, repo: &Path) -> Result<()> {
    let ctx = Ctx::new(env, repo);
    let (_, steps, _) = provision::load_wanted(&ctx)?;
    let items = survey(&ctx, &steps, &[], true);
    if items.iter().any(Item::behind) {
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
/// one confirmation; the shell is then restarted if a plugin moved (not during a screen
/// recording). Returns the exit code: 0, or 1 if one failed.
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
    let todo: Vec<Item> = survey(&ctx, &steps, ids, false)
        .into_iter()
        .filter(Item::behind)
        .collect();
    if todo.is_empty() {
        println!("\nNothing to update.");
        return Ok(0);
    }
    println!("\nTo update");
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
    if todo.iter().any(|i| matches!(i, Item::Plugin { .. })) {
        println!("\nthen: omarchy restart shell (not during a screen recording)");
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
        // Already confirmed with the update.
        if !reload::restart_shell() {
            failed.push("shell restart (plugins not loaded)".into());
        }
    }
    if !failed.is_empty() {
        eprintln!("\nfailed: {}", failed.join(", "));
        return Ok(1);
    }
    Ok(0)
}
