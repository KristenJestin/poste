//! `poste-files`: fast file search, without a daemon. The index is a file (walk with the
//! `ignore` crate, exclusions in .gitignore syntax), rebuilt in the background at each SUPER+D;
//! `search` reads it again and searches it (neo_frizbee), in the island launcher's format.

mod config;
mod desktop;
mod index;
mod item;
mod recent;
mod rules;
mod store;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::Config;
use poste_core::Env;
use serde_json::Value;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

#[derive(Parser)]
#[command(
    name = "poste files",
    bin_name = "poste files",
    version,
    about = "Fast file search (index on disk, no daemon)"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Search for a file or folder
    Search {
        /// Words of the query; may start with "-" (the launcher passes what was typed)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        query: Vec<String>,
        /// Output in the island launcher's format
        #[arg(long)]
        json: bool,
        /// Number of results (default: `limit` in files.toml)
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Recent files, shown by the launcher before anything is typed
    Home {
        #[arg(long)]
        json: bool,
    },
    /// Rebuild the index (unless it is under 10 s old or a walk is already running)
    Refresh {
        /// Return at once, the walk goes on by itself (SUPER+D)
        #[arg(long)]
        background: bool,
        /// Even when the index is recent
        #[arg(long)]
        force: bool,
        /// Started by `--background`: clears the pending marker at the end
        #[arg(long, hide = true)]
        spawned: bool,
    },
    /// Open a file with its default application and remember it in the recent files
    Open { path: PathBuf },
    /// Copy the file itself to the clipboard (to paste it as a file)
    Copy { path: PathBuf },
    /// Show a file in the default file manager
    Reveal { path: PathBuf },
    /// State of the index
    Status,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let env = match Env::from_process() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poste files: {e:#}");
            return ExitCode::FAILURE;
        }
    };
    let json = matches!(
        cli.command,
        Cmd::Search { json: true, .. } | Cmd::Home { json: true }
    );
    match run(&env, cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        // With --json, a failure gives a single launcher entry and exit code 0.
        Err(e) if json => {
            print_json(&[item::notice(
                "files-error",
                "File search unavailable",
                &format!("{e:#}"),
                "dialog-error",
            )]);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("poste files: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn print_json(v: &[Value]) {
    println!("{}", Value::Array(v.to_vec()));
}

fn print_hits(env: &Env, hits: &[index::Hit], json: bool) {
    if json {
        let out: Vec<Value> = hits.iter().map(|h| item::to_json(env, h)).collect();
        print_json(&out);
    } else if hits.is_empty() {
        println!("No results");
    } else {
        for h in hits {
            println!("{}{}", h.path, if h.is_dir { "/" } else { "" });
        }
    }
}

/// "3 min ago", for the age of the index.
fn ago(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", s / 60),
        3600..=86399 => format!("{} h ago", s / 3600),
        _ => format!("{} days ago", s / 86400),
    }
}

/// Runs `program arg` detached (own process group, no output), without waiting for it.
fn detach(program: &str, arg: &std::path::Path) -> Result<()> {
    Command::new(program)
        .arg(arg)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()?;
    Ok(())
}

fn run(env: &Env, cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Search { query, json, limit } => {
            let query = query.join(" ");
            let limit = match limit {
                Some(l) => l.clamp(1, 200),
                None => Config::load(env)?.limit,
            };
            let hits = store::load(env)?.search(&query, limit);
            // Results first, housekeeping after.
            print_hits(env, &hits, json);
            if let Err(e) = store::refresh_if_stale(env) {
                eprintln!("poste files: {e:#}");
            }
        }
        Cmd::Home { json } => {
            // Nothing typed: the recent files, or nothing at all (no hint line).
            let hits = recent::list(env, Config::load(env)?.limit);
            print_hits(env, &hits, json);
        }
        Cmd::Refresh {
            background,
            force,
            spawned,
        } => {
            if background {
                store::refresh_in_background(env)?;
            } else {
                let done = store::refresh(env, force);
                if spawned {
                    store::clear_pending(env);
                }
                match done? {
                    store::Refresh::Done(m) => {
                        println!("{} entries indexed in {} ms", m.count, m.scan_ms)
                    }
                    store::Refresh::Skipped => {
                        println!("Index recent or walk already running: nothing to do")
                    }
                }
            }
        }
        Cmd::Open { path } => {
            if let Err(e) = recent::record(env, &path) {
                eprintln!("poste files: {e:#}");
            }
            detach("xdg-open", &path)?;
        }
        Cmd::Copy { path } => desktop::copy_file(&std::path::absolute(&path)?)?,
        Cmd::Reveal { path } => {
            let path = std::path::absolute(&path)?;
            if let Err(e) = desktop::show_item(&path) {
                // No file manager answers on the bus: open the folder with the default
                // application for folders, without naming any.
                eprintln!("poste files: {e:#}");
                detach("xdg-open", path.parent().unwrap_or(&path))?;
            }
        }
        Cmd::Status => {
            let cfg = Config::load(env)?;
            let roots: Vec<String> = cfg
                .root_paths(env)?
                .iter()
                .map(|r| env.display(r))
                .collect();
            println!("Roots: {}", roots.join(", "));
            match store::peek(env) {
                Some((age, m)) => println!(
                    "Index: {} entries, rebuilt {} (walk in {} ms)",
                    m.count,
                    ago(age),
                    m.scan_ms
                ),
                None if store::index_path(env).exists() => {
                    println!("Index: unreadable, rebuilt at the next search")
                }
                None => println!("Index: not built yet"),
            }
            println!("File: {}", env.display(&store::index_path(env)));
            println!("Settings: {}", env.display(&Config::file(env)));
            for f in Config::ignore_files(env) {
                let state = if f.exists() { "" } else { " (none)" };
                println!("Exclusions: {}{state}", env.display(&f));
            }
        }
    }
    Ok(())
}
