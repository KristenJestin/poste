//! `poste-sys`: management of the poste repository (build, install, status, git).

mod apps;
mod config;
mod handoff;
mod ignore;
mod import;
mod lines;
mod machine;
mod manifest;
mod ojson;
mod orphans;
mod overrides;
mod plan;
mod provision;
mod reload;
mod setup;
mod status;
mod sync;
mod units;
mod updates;
mod util;
mod versions;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use poste_core::{Config, Env};
use std::process::{Command, ExitCode};
use versions::Fetcher;

#[derive(Parser)]
#[command(
    name = "poste sys",
    bin_name = "poste sys",
    version,
    about = "Manages the poste repository"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Builds the repository in release then installs poste and its modules
    Setup {
        /// Applies without asking for confirmation
        #[arg(short, long)]
        yes: bool,
        /// Shows the plan without writing anything
        #[arg(short = 'n', long)]
        dry_run: bool,
        /// Does not touch the config (binaries and module files only)
        #[arg(long)]
        no_config: bool,
        /// Config conflict: the repository wins (the old one is backed up), repeatable
        #[arg(long, value_name = "PATH")]
        take_repo: Vec<String>,
        /// Config conflict: the file of ~ is copied into the repository, repeatable
        #[arg(long, value_name = "PATH")]
        take_home: Vec<String>,
    },
    /// Compares the repository's config with the one in ~ (one path, or all the differences)
    Diff {
        /// Path relative to ~ (or absolute, or ~/…)
        path: Option<String>,
    },
    /// Puts a file of ~ under management: moved into config/, replaced by a link
    Add {
        path: String,
        /// Shows what would be done without writing anything
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Puts an ordinary file back in place of the link (the repository keeps its file)
    Forget { path: String },
    /// Updates according to the source: dev, git pull of the clone; release, latest published version (verified). Then git pull of the data repository, setup (preview included), reload of the island and Hyprland if needed
    Sync {
        #[arg(short, long)]
        yes: bool,
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Imports the config managed by chezmoi into config/ (real state of the machine)
    ImportChezmoi {
        /// Source folder of chezmoi (default: chezmoi's own)
        #[arg(long)]
        source: Option<std::path::PathBuf>,
        /// Writes the report to the output, nothing in the repository
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Marks a manual step (or a system step that cannot be checked) done; without id, lists them
    Done {
        id: Option<String>,
        /// Puts the step back to do
        #[arg(long)]
        undo: bool,
    },
    /// State of the repository, the installed modules and the files put in place
    Status,
    /// Runs `git -C <repository> <args…>`
    Git {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Config files this machine keeps for itself (config.toml, key local)
    Local {
        #[command(subcommand)]
        action: Option<LocalCmd>,
    },
    /// JSON overrides of this machine (config.toml, [[override]]): generated file, not linked
    Override {
        #[command(subcommand)]
        action: Option<OverrideCmd>,
    },
    /// Modules of the repository not installed on this machine (config.toml, key disabled)
    Module {
        #[command(subcommand)]
        action: Option<ModuleCmd>,
    },
    /// Add-ons of the repository and their state here (key addons, in poste.toml of the data repository)
    Addons,
    /// Bootstrap packages and steps ignored on this machine (config.toml, key skip)
    Skip {
        #[command(subcommand)]
        action: Option<SkipCmd>,
    },
    /// Switches the machine: dev (the clone), release (the latest installed version), or a version (start of the commit)
    Use {
        /// dev, release, or the start of the commit of an installed version
        target: String,
        #[arg(short, long)]
        yes: bool,
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Source, root (current) and installed versions
    Versions,
    /// What is behind its source: apps (GitHub releases) and Omarchy plugins
    Outdated,
    /// Updates the apps and plugins that are behind (all, or the cited ones), after a preview
    Update {
        /// Identifiers of apps or plugins (default: all those behind)
        ids: Vec<String>,
        #[arg(short, long)]
        yes: bool,
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Continuation of sync and use, run by an old poste-sys that hands over (see handoff)
    #[command(hide = true)]
    Finish {
        #[arg(short, long)]
        yes: bool,
        /// Removes the old versions after setup
        #[arg(long)]
        prune: bool,
        /// The clone is already built by the one that hands over
        #[arg(long)]
        skip_build: bool,
        /// The previous root: what must be reloaded is computed again from the two roots
        #[arg(long)]
        from: Option<std::path::PathBuf>,
        #[arg(long)]
        reload_shell: bool,
        #[arg(long)]
        reload_hypr: bool,
    },
    /// Prints the path of the repository (cd "$(poste sys cd)")
    Cd,
}

#[derive(Subcommand)]
enum LocalCmd {
    /// Lists the local files
    List,
    /// Keeps this path of config/ to this machine: setup neither links nor touches it
    Add { path: String },
    /// Puts it back under the repository's management
    Remove { path: String },
}

#[derive(Subcommand)]
enum OverrideCmd {
    /// Lists the overrides
    List,
    /// Sets a dotted key (JSON value: 3600, true, "text") in a JSON file of config/
    Set {
        path: String,
        key: String,
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    /// Removes an override (the last one removed links the file to the repository again)
    Unset { path: String, key: String },
}

#[derive(Subcommand)]
enum ModuleCmd {
    /// Lists the modules of the repository and their state here
    List,
    /// Enables a module again (installed at the next setup)
    Enable { name: String },
    /// Disables a module (removed at the next setup, with a preview)
    Disable { name: String },
}

#[derive(Subcommand)]
enum SkipCmd {
    /// Lists the ignored identifiers
    List,
    /// Ignores packages (packages.toml) or steps (steps.toml) here; nothing is uninstalled
    Add {
        #[arg(required = true)]
        ids: Vec<String>,
    },
    /// Stops ignoring these identifiers
    Remove {
        #[arg(required = true)]
        ids: Vec<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("poste sys: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let env = Env::from_process()?;
    let cfg = Config::load(&env)?;
    match cli.command {
        Cmd::Setup {
            yes,
            dry_run,
            no_config,
            take_repo,
            take_home,
        } => {
            let repo = versions::ensure_current(&env, &cfg, dry_run)?;
            if !take_home.is_empty() {
                versions::ensure_writable(&env, &repo, "--take-home")?;
            }
            let built = handoff::setup_from_clone(&repo)?;
            let done = setup::run(
                &env,
                &repo,
                &setup::Opts {
                    yes,
                    dry_run,
                    no_config,
                    skip_build: built,
                    link_root: versions::link_root(&env, &repo),
                    take_repo,
                    take_home,
                },
            )?;
            if done.config_changed {
                println!(
                    "\nConfig changed: to reload it, run `omarchy restart shell` and `hyprctl reload` yourself."
                );
            }
            Ok(done.code)
        }
        Cmd::Diff { path } => {
            let repo = versions::root(&env, &cfg)?;
            let Some(layout) = config::load(&repo, &env)? else {
                bail!("no config/ folder in the repository");
            };
            config::diff(&layout, &env, path.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Add { path, dry_run } => {
            let repo = versions::root(&env, &cfg)?;
            versions::ensure_writable(&env, &repo, "add")?;
            config::add(&env, &repo, &path, dry_run, &env.manifest_file())?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Forget { path } => {
            let repo = versions::root(&env, &cfg)?;
            let Some(layout) = config::load(&repo, &env)? else {
                bail!("no config/ folder in the repository");
            };
            config::forget(&layout, &env, &path, &env.manifest_file())?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Sync { yes, dry_run } => {
            let done = sync::run(&env, &cfg, &Fetcher::from_env(), yes, dry_run)?;
            reload::apply(&done.need, yes, dry_run, &setup::confirm)?;
            Ok(done.code)
        }
        Cmd::Use {
            target,
            yes,
            dry_run,
        } => {
            let done = versions::use_target(
                &env,
                &cfg,
                &versions::Target::parse(&target),
                &Fetcher::from_env(),
                &setup::Opts {
                    yes,
                    dry_run,
                    ..Default::default()
                },
            )?;
            reload::apply(&done.need, yes, dry_run, &setup::confirm)?;
            Ok(done.code)
        }
        Cmd::Finish {
            yes,
            prune,
            skip_build,
            from,
            reload_shell,
            reload_hypr,
        } => {
            let mut need = reload::Need {
                shell: reload_shell,
                hypr: reload_hypr,
            };
            let root = env.current_link();
            if let Some(old) = &from {
                versions::add_root_changes(&mut need, Some(old), &root)?;
            }
            let opts = setup::Opts {
                yes,
                skip_build,
                ..Default::default()
            };
            let done = versions::finish(&env, &root, need, &opts, prune)?;
            reload::apply(&done.need, yes, false, &setup::confirm)?;
            Ok(done.code)
        }
        Cmd::Outdated => {
            let repo = versions::root(&env, &cfg)?;
            updates::outdated(&env, &repo)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Update { ids, yes, dry_run } => {
            let repo = versions::root(&env, &cfg)?;
            let code = updates::update(&env, &repo, &ids, yes, dry_run, &setup::confirm)?;
            Ok(ExitCode::from(code))
        }
        Cmd::Versions => {
            versions::list(&env, &cfg)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::ImportChezmoi { source, dry_run } => {
            let repo = util::clone(&env, &cfg)?;
            import::run(&env, &repo, source.as_deref(), dry_run)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Done { id, undo } => {
            let repo = versions::root(&env, &cfg)?;
            provision::done(&env, &repo, id.as_deref(), undo)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Status => {
            let repo = versions::root(&env, &cfg)?;
            status::run(&env, &cfg, &repo)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Git { args } => {
            let repo = util::clone(&env, &cfg)?;
            let mut cmd = Command::new("git");
            cmd.arg("-C").arg(&repo).args(args);
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                Err(anyhow::Error::new(cmd.exec()).context("cannot run git"))
            }
            #[cfg(not(unix))]
            {
                let s = cmd.status()?;
                Ok(ExitCode::from(s.code().unwrap_or(1) as u8))
            }
        }
        Cmd::Local { action } => {
            let repo = versions::root(&env, &cfg)?;
            match action.unwrap_or(LocalCmd::List) {
                LocalCmd::List => machine::local_list(&env, &repo)?,
                LocalCmd::Add { path } => machine::local_add(&env, &repo, &path)?,
                LocalCmd::Remove { path } => machine::local_remove(&env, &path)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Override { action } => {
            let repo = versions::root(&env, &cfg)?;
            match action.unwrap_or(OverrideCmd::List) {
                OverrideCmd::List => machine::override_list(&env, &repo)?,
                OverrideCmd::Set { path, key, value } => {
                    machine::override_set(&env, &repo, &path, &key, &value)?
                }
                OverrideCmd::Unset { path, key } => machine::override_unset(&env, &path, &key)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Module { action } => {
            let repo = versions::root(&env, &cfg)?;
            match action.unwrap_or(ModuleCmd::List) {
                ModuleCmd::List => machine::module_list(&env, &repo)?,
                ModuleCmd::Enable { name } => machine::module_set(&env, &repo, &name, true)?,
                ModuleCmd::Disable { name } => machine::module_set(&env, &repo, &name, false)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Addons => {
            let repo = versions::root(&env, &cfg)?;
            machine::addon_list(&env, &repo)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Skip { action } => {
            let repo = versions::root(&env, &cfg)?;
            match action.unwrap_or(SkipCmd::List) {
                SkipCmd::List => machine::skip_list(&env, &repo)?,
                SkipCmd::Add { ids } => machine::skip_add(&env, &repo, &ids)?,
                SkipCmd::Remove { ids } => machine::skip_remove(&env, &ids)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Cd => {
            println!("{}", util::clone(&env, &cfg)?.display());
            Ok(ExitCode::SUCCESS)
        }
    }
}
