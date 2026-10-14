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
mod reload;
mod setup;
mod status;
mod sync;
mod units;
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
    /// Met à jour selon la source : dev, git pull du clone ; release, dernière version publiée (vérifiée). Puis git pull du dépôt de données, setup (aperçu compris), rechargement de l'île et de Hyprland si besoin
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
    /// Paquets et étapes du bootstrap ignorés sur cette machine (config.toml, clé skip)
    Skip {
        #[command(subcommand)]
        action: Option<SkipCmd>,
    },
    /// Bascule la machine : dev (le clone), release (la dernière version installée), ou une version (début du commit)
    Use {
        /// dev, release, ou le début du commit d'une version installée
        target: String,
        #[arg(short, long)]
        yes: bool,
        #[arg(short = 'n', long)]
        dry_run: bool,
    },
    /// Source, racine (current) et versions installées
    Versions,
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

#[derive(Subcommand)]
enum SkipCmd {
    /// Liste les identifiants ignorés
    List,
    /// Ignore des paquets (packages.toml) ou des étapes (steps.toml) ici ; rien n'est désinstallé
    Add {
        #[arg(required = true)]
        ids: Vec<String>,
    },
    /// Cesse d'ignorer ces identifiants
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
            let repo = versions::ensure_current(&env, &cfg, dry_run)?;
            if !take_home.is_empty() {
                versions::ensure_writable(&env, &repo, "--take-home")?;
            }
            let done = setup::run(
                &env,
                &repo,
                &setup::Opts {
                    yes,
                    dry_run,
                    no_config,
                    skip_build: false,
                    link_root: versions::link_root(&env, &repo),
                    take_repo,
                    take_home,
                },
            )?;
            if done.config_changed {
                println!(
                    "\nConfig modifiée : pour la recharger, lancez vous-même `omarchy restart shell` et `hyprctl reload`."
                );
            }
            Ok(done.code)
        }
        Cmd::Diff { path } => {
            let repo = versions::root(&env, &cfg)?;
            let Some(layout) = config::load(&repo, &env)? else {
                bail!("pas de dossier config/ dans le dépôt");
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
                bail!("pas de dossier config/ dans le dépôt");
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
                Err(anyhow::Error::new(cmd.exec()).context("impossible de lancer git"))
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
