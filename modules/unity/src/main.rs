//! `poste-unity`: the Unity projects of the machine, for the island launcher's
//! "Unity Projects" extension.

mod projects;

use anyhow::Result;
use clap::{Parser, Subcommand};
use poste_core::Env;
use projects::Config;
use serde_json::Value;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "poste unity",
    bin_name = "poste unity",
    version,
    about = "Unity projects in the launcher"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The Unity projects, favorites first, then the newest
    Projects {
        /// Output in the island launcher's format
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let Cli {
        command: Cmd::Projects { json },
    } = Cli::parse();
    match run(json) {
        Ok(()) => ExitCode::SUCCESS,
        // With --json, a failure gives a single launcher entry and exit code 0.
        Err(e) if json => {
            let v = projects::notice("Unity projects unavailable", &format!("{e:#}"));
            println!("{}", Value::Array(vec![v]));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("poste unity: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(json: bool) -> Result<()> {
    let env = Env::from_process()?;
    let list = projects::list(&env, &Config::load(&env)?);
    if json {
        let out: Vec<Value> = list.iter().map(|p| projects::to_json(&env, p)).collect();
        println!("{}", Value::Array(out));
    } else if list.is_empty() {
        println!("No Unity project");
    } else {
        for p in &list {
            println!("{}\t{}", env.display(&p.path), p.version);
        }
    }
    Ok(())
}
