//! Module record de poste : enregistrement d'écran avec gpu-screen-recorder, piloté par l'île.
//! Se lance par `poste record <commande>` (aide : voir `usage.txt`).

pub mod audio;
pub mod commands;
pub mod env;
pub mod hypr;
pub mod items;
pub mod picker;
pub mod proc;
pub mod pyval;
pub mod state;

use env::Env;

/// Valeur de `--preset` dans `rest`, `Err` si le drapeau est le dernier argument.
fn preset_arg(rest: &[String]) -> Result<Option<String>, ()> {
    match rest.iter().position(|a| a == "--preset") {
        None => Ok(None),
        Some(i) => rest.get(i + 1).cloned().map(Some).ok_or(()),
    }
}

pub fn run(argv: &[String]) -> i32 {
    let env = Env::from_env();
    let Some(cmd) = argv.first() else {
        println!("{}", commands::USAGE);
        return 2;
    };
    let rest = &argv[1..];
    match cmd.as_str() {
        // Superviseur interne (lancé par `start`).
        "--supervise" => match rest.first() {
            Some(spec) => {
                commands::supervise(&env, spec);
                0
            }
            None => 2,
        },
        // Sélecteur de partage d'écran de xdph (appelé par bin/poste-share-picker).
        "share-picker" => picker::run(&env, rest),
        "start" => commands::start(&env, rest),
        "stop" => commands::stop(&env),
        "toggle" => commands::pause(&env, None),
        "pause" => commands::pause(&env, Some(true)),
        "resume" => commands::pause(&env, Some(false)),
        "status" => {
            println!(
                "{}",
                pyval::dumps(&state::public(state::read_state(&env).as_ref()))
            );
            0
        }
        "watch" => commands::watch(&env),
        "open-dir" | "open-last" => {
            let Ok(preset) = preset_arg(rest) else {
                eprintln!("poste-record : --preset demande une valeur");
                return 1;
            };
            if cmd == "open-dir" {
                commands::open_dir(&env, preset.as_deref())
            } else {
                commands::open_last(&env, preset.as_deref())
            }
        }
        "items" => {
            println!("{}", items::items_json(&env));
            0
        }
        "hotkey" => commands::hotkey(&env),
        _ => {
            println!("{}", commands::USAGE);
            2
        }
    }
}
