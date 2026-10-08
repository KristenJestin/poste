//! `poste` : aiguillage mince. `poste <module> <args…>` exécute `poste-<module> <args…>`.
//! Seuls `help`, `--help` et `--version` sont connus ici ; aucun nom de module n'est codé.

use poste_core::{modules, Config, Env, Module};
use std::ffi::OsString;
use std::process::{Command, ExitCode};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let env = match Env::from_process() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poste : {e:#}");
            return ExitCode::from(1);
        }
    };
    // Une config illisible ne doit pas empêcher d'utiliser les modules du PATH.
    let cfg = Config::load(&env).unwrap_or_else(|e| {
        eprintln!("poste : avertissement : {e:#}");
        Config::default()
    });

    let mut args = std::env::args_os().skip(1);
    let Some(first) = args.next() else {
        print_help(&env, &cfg);
        return ExitCode::SUCCESS;
    };

    match first.to_str() {
        Some("help" | "--help" | "-h") => {
            match args.next().as_deref().and_then(|a| a.to_str()) {
                Some(name) => return help_module(&env, &cfg, name),
                None => print_help(&env, &cfg),
            }
            ExitCode::SUCCESS
        }
        Some("--version" | "-V") => {
            println!("poste {VERSION}");
            ExitCode::SUCCESS
        }
        Some(name) => match modules::find(&env, &cfg, name) {
            Some(m) => run(&m, args.collect()),
            None => {
                eprintln!("poste : module « {name} » introuvable.\n");
                print_modules(&env, &cfg, true);
                ExitCode::from(127)
            }
        },
        None => {
            eprintln!("poste : nom de module invalide (non UTF-8).");
            ExitCode::from(2)
        }
    }
}

/// Remplace le processus par le module (Unix) ; sinon le lance et transmet son code.
fn run(m: &Module, args: Vec<OsString>) -> ExitCode {
    let mut cmd = Command::new(&m.program);
    cmd.args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        eprintln!(
            "poste : impossible de lancer {} : {err}",
            m.program.display()
        );
        ExitCode::from(126)
    }
    #[cfg(not(unix))]
    match cmd.status() {
        Ok(s) => ExitCode::from(s.code().unwrap_or(1) as u8),
        Err(e) => {
            eprintln!("poste : impossible de lancer {} : {e}", m.program.display());
            ExitCode::from(126)
        }
    }
}

fn print_help(env: &Env, cfg: &Config) {
    println!("poste {VERSION} : environnement de travail\n");
    println!("Usage :");
    println!("  poste <module> [args…]   exécute poste-<module> avec ces arguments");
    println!("  poste help [module]      cette aide, ou les commandes d'un module");
    println!("  poste --version\n");
    print_modules(env, cfg, false);
}

/// Liste les modules ; `to_stderr` pour le message d'erreur d'un module inconnu.
fn print_modules(env: &Env, cfg: &Config, to_stderr: bool) {
    let mods = modules::list(env, cfg);
    let mut out = String::new();
    if mods.is_empty() {
        out.push_str("Aucun module trouvé.\n");
    } else {
        out.push_str("Modules disponibles :\n");
        let width = mods
            .iter()
            .map(|m| m.name.chars().count())
            .max()
            .unwrap_or(0);
        for m in &mods {
            out.push_str(&format!("  {:<width$}  {}\n", m.name, m.description()));
        }
    }
    if to_stderr {
        eprint!("{out}");
    } else {
        print!("{out}");
    }
}

fn help_module(env: &Env, cfg: &Config, name: &str) -> ExitCode {
    let Some(m) = modules::find(env, cfg, name) else {
        eprintln!("poste : module « {name} » introuvable.\n");
        print_modules(env, cfg, true);
        return ExitCode::from(127);
    };
    println!("poste {name}  {}", m.description());
    println!("programme : {}", env.display(&m.program));
    if let Some(card) = &m.card {
        if !card.commands.is_empty() {
            println!("\nCommandes :");
            let width = card
                .commands
                .iter()
                .map(|c| c.name.chars().count())
                .max()
                .unwrap_or(0);
            for c in &card.commands {
                println!("  {:<width$}  {}", c.name, c.description);
            }
        }
    }
    println!("\nDétails : poste {name} --help");
    ExitCode::SUCCESS
}
