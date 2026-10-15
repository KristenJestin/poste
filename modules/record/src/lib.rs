//! Record module of poste: screen recording with gpu-screen-recorder, driven by the island.
//! Run with `poste record <command>` (help: see `usage.txt`).

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

/// Value of `--preset` in `rest`, `Err` if the flag is the last argument.
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
        // Internal supervisor (launched by `start`).
        "--supervise" => match rest.first() {
            Some(spec) => {
                commands::supervise(&env, spec);
                0
            }
            None => 2,
        },
        // xdph screen-share picker (called by bin/poste-share-picker).
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
                eprintln!("poste-record: --preset needs a value");
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
