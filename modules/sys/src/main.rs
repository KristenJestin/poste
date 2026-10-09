//! `poste-sys` : gestion du dépôt poste (construction, installation, état, git).

mod config;
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
mod setup;
mod status;
mod units;
mod util;

use anyhow::{bail, Context, Result};
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
        /// Ne touche pas à la config (binaires et fichiers des modules seulement)
        #[arg(long)]
        no_config: bool,
        /// Conflit de config : le dépôt gagne (l'ancien est sauvegardé), répétable
        #[arg(long, value_name = "CHEMIN")]
        take_repo: Vec<String>,
        /// Conflit de config : le fichier de ~ est recopié dans le dépôt, répétable
        #[arg(long, value_name = "CHEMIN")]
        take_home: Vec<String>,
    },
    /// Compare la config du dépôt à celle de ~ (un chemin, ou tous les écarts)
    Diff {
        /// Chemin relatif à ~ (ou absolu, ou ~/…)
        path: Option<String>,
    },
    /// Met un fichier de ~ sous gestion : déplacé dans config/, remplacé par un lien
    Add {
        path: String,
        /// Montre ce qui serait fait sans rien écrire
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Remet un fichier ordinaire à la place du lien (le dépôt garde son fichier)
    Forget { path: String },
    /// git pull --ff-only, puis setup (aperçu compris)
    Sync {
        #[arg(short, long)]
        yes: bool,
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Importe la config gérée par chezmoi dans config/ (état réel de la machine)
    ImportChezmoi {
        /// Dossier source de chezmoi (défaut : celui de chezmoi)
        #[arg(long)]
        source: Option<std::path::PathBuf>,
        /// Écrit le rapport sur la sortie, rien dans le dépôt
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Marque une étape manuelle (ou système non vérifiable) faite ; sans id, les liste
    Done {
        id: Option<String>,
        /// Remet l'étape à faire
        #[arg(long)]
        undo: bool,
    },
    /// État du dépôt, des modules installés et des fichiers posés
    Status,
    /// Lance `git -C <dépôt> <args…>`
    Git {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Fichiers de config que cette machine garde à elle (config.toml, clé local)
    Local {
        #[command(subcommand)]
        action: Option<LocalCmd>,
    },
    /// Surcharges JSON de cette machine (config.toml, [[override]]) : fichier généré, pas lié
    Override {
        #[command(subcommand)]
        action: Option<OverrideCmd>,
    },
    /// Modules du dépôt non installés sur cette machine (config.toml, clé disabled)
    Module {
        #[command(subcommand)]
        action: Option<ModuleCmd>,
    },
    /// Affiche le chemin du dépôt (cd "$(poste sys cd)")
    Cd,
}

#[derive(Subcommand)]
enum LocalCmd {
    /// Liste les fichiers locaux
    List,
    /// Garde ce chemin de config/ à cette machine : setup ne le lie ni ne le touche
    Add { path: String },
    /// Le remet sous la gestion du dépôt
    Remove { path: String },
}

#[derive(Subcommand)]
enum OverrideCmd {
    /// Liste les surcharges
    List,
    /// Fixe une clé pointée (valeur JSON : 3600, true, "texte") dans un fichier JSON de config/
    Set {
        path: String,
        key: String,
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    /// Retire une surcharge (la dernière retirée relie le fichier au dépôt)
    Unset { path: String, key: String },
}

#[derive(Subcommand)]
enum ModuleCmd {
    /// Liste les modules du dépôt et leur état ici
    List,
    /// Réactive un module (installé au prochain setup)
    Enable { name: String },
    /// Désactive un module (retiré au prochain setup, avec aperçu)
    Disable { name: String },
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
        Cmd::Setup {
            yes,
            dry_run,
            no_config,
            take_repo,
            take_home,
        } => {
            let repo = util::repo(&env, &cfg)?;
            setup::run(
                &env,
                &repo,
                &setup::Opts {
                    yes,
                    dry_run,
                    no_config,
                    skip_build: false,
                    take_repo,
                    take_home,
                },
            )
        }
        Cmd::Diff { path } => {
            let repo = util::repo(&env, &cfg)?;
            let Some(layout) = config::load(&repo, &env)? else {
                bail!("pas de dossier config/ dans le dépôt");
            };
            config::diff(&layout, &env, path.as_deref())?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Add { path, dry_run } => {
            let repo = util::repo(&env, &cfg)?;
            config::add(&env, &repo, &path, dry_run, &env.manifest_file())?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Forget { path } => {
            let repo = util::repo(&env, &cfg)?;
            let Some(layout) = config::load(&repo, &env)? else {
                bail!("pas de dossier config/ dans le dépôt");
            };
            config::forget(&layout, &env, &path, &env.manifest_file())?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Sync { yes, dry_run } => {
            let repo = util::repo(&env, &cfg)?;
            if dry_run {
                println!("Simulation : git pull --ff-only non lancé.");
            } else {
                let st = Command::new("git")
                    .arg("-C")
                    .arg(&repo)
                    .args(["pull", "--ff-only"])
                    .status()
                    .context("impossible de lancer git")?;
                if !st.success() {
                    bail!("git pull --ff-only a échoué, setup non lancé");
                }
            }
            setup::run(
                &env,
                &repo,
                &setup::Opts {
                    yes,
                    dry_run,
                    ..Default::default()
                },
            )
        }
        Cmd::ImportChezmoi { source, dry_run } => {
            let repo = util::repo(&env, &cfg)?;
            import::run(&env, &repo, source.as_deref(), dry_run)?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Done { id, undo } => {
            let repo = util::repo(&env, &cfg)?;
            provision::done(&env, &repo, id.as_deref(), undo)?;
            Ok(ExitCode::SUCCESS)
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
        Cmd::Local { action } => {
            let repo = util::repo(&env, &cfg)?;
            match action.unwrap_or(LocalCmd::List) {
                LocalCmd::List => machine::local_list(&env, &repo)?,
                LocalCmd::Add { path } => machine::local_add(&env, &repo, &path)?,
                LocalCmd::Remove { path } => machine::local_remove(&env, &path)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Override { action } => {
            let repo = util::repo(&env, &cfg)?;
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
            let repo = util::repo(&env, &cfg)?;
            match action.unwrap_or(ModuleCmd::List) {
                ModuleCmd::List => machine::module_list(&env, &repo)?,
                ModuleCmd::Enable { name } => machine::module_set(&env, &repo, &name, true)?,
                ModuleCmd::Disable { name } => machine::module_set(&env, &repo, &name, false)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Cd => {
            println!("{}", util::repo(&env, &cfg)?.display());
            Ok(ExitCode::SUCCESS)
        }
    }
}
