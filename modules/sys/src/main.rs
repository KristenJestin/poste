//! `poste-sys` : gestion du dépôt poste (construction, installation, état, git).

mod manifest;
mod plan;
mod setup;
mod status;
mod util;

use anyhow::Result;
use clap::{Parser, Subcommand};
use poste_core::{Config, Env};
use std::process::{Command, ExitCode};

#[derive(Parser)]
#[command(
    name = "poste sys",
    bin_name = "poste sys",
    version,
    about = "Gère le dépôt poste"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Construit le dépôt en release puis installe poste et ses modules
    Setup {
        /// Applique sans demander confirmation
        #[arg(short, long)]
        yes: bool,
        /// Affiche le plan sans rien écrire
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// État du dépôt, des modules installés et des fichiers posés
    Status,
    /// Lance `git -C <dépôt> <args…>`
    Git {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Affiche le chemin du dépôt (cd "$(poste sys cd)")
    Cd,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("poste sys : {e:#}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let env = Env::from_process()?;
    let cfg = Config::load(&env)?;
    match cli.command {
        Cmd::Setup { yes, dry_run } => {
            let repo = util::repo(&env, &cfg)?;
            setup::run(&env, &repo, yes, dry_run)
        }
        Cmd::Status => {
            let repo = util::repo(&env, &cfg)?;
            status::run(&env, &repo)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Git { args } => {
            let repo = util::repo(&env, &cfg)?;
            let mut cmd = Command::new("git");
            cmd.arg("-C").arg(&repo).args(args);
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                Err(anyhow::Error::new(cmd.exec()).context("impossible de lancer git"))
            }
            #[cfg(not(unix))]
            {
                let s = cmd.status()?;
                Ok(ExitCode::from(s.code().unwrap_or(1) as u8))
            }
        }
        Cmd::Cd => {
            println!("{}", util::repo(&env, &cfg)?.display());
            Ok(ExitCode::SUCCESS)
        }
    }
}
