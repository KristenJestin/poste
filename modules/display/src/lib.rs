//! Module display de poste : profils d'écrans Hyprland choisis selon les écrans branchés.
//! Se lance par `poste display <commande>` (aide : `USAGE`).

pub mod config;
pub mod hypr;
pub mod model;
pub mod notify;
pub mod plan;

use config::{Config, Profile};
use model::{parse_monitors, Monitor};
use poste_core::Env;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const USAGE: &str = "Module display de poste : profils d'écrans Hyprland.

  poste display status              profil courant, écrans branchés, profil que choisirait « auto »
  poste display list                profils déclarés (~/.config/poste/display.toml)
  poste display apply <profil> [--dry-run]   applique un profil (--dry-run : affiche les commandes)
  poste display auto [--dry-run]    choisit et applique le profil qui correspond aux écrans branchés
  poste display items               liste de l'extension « Écrans » du lanceur (JSON)
  poste display watch               le démon : écoute les branchements d'écrans et applique « auto »

Profils : voir display.toml.example du module.";

// ---------------------------------------------------------------- état

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Dernier profil appliqué (`None` : aucun ne correspondait).
    pub profile: Option<String>,
    /// Empreinte des écrans au moment de la décision.
    pub sig: String,
    /// Appliqué à la main (`apply`) : `auto` n'y touche pas tant que les écrans ne changent pas.
    #[serde(default)]
    pub manual: bool,
}

/// `$XDG_RUNTIME_DIR/poste-display/state.json` (perdu au redémarrage), sinon sous le dossier d'état.
pub fn state_file(env: &Env) -> PathBuf {
    match std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(d) => PathBuf::from(d).join("poste-display/state.json"),
        None => env.state_dir().join("display-state.json"),
    }
}

pub fn load_state(env: &Env) -> Option<State> {
    serde_json::from_str(&std::fs::read_to_string(state_file(env)).ok()?).ok()
}

fn save_state(env: &Env, st: &State) {
    let f = state_file(env);
    if let Some(d) = f.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = f.with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_string(st).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, &f);
    }
}

// ---------------------------------------------------------------- application

fn monitors() -> Result<Vec<Monitor>, String> {
    let json = hypr::monitors_json().ok_or("hyprctl ne répond pas (Hyprland est-il lancé ?)")?;
    parse_monitors(&json).map_err(|e| format!("réponse de hyprctl illisible : {e}"))
}

/// Applique (ou montre) un profil. Renvoie le nombre de commandes refusées.
fn apply_profile(
    env: &Env,
    cfg: &Config,
    p: &Profile,
    mons: &[Monitor],
    dry_run: bool,
    manual: bool,
) -> usize {
    let clients = hypr::clients_json();
    let cmds = plan::commands(cfg, p, mons, &clients);
    if dry_run {
        println!("Profil {} (simulation, rien n'est appliqué) :", p.label());
        for c in &cmds {
            println!("  {}\n      hyprctl eval '{}'", c.desc, c.lua);
            if let Some(l) = &c.legacy {
                println!("      repli : hyprctl {}", l.join(" "));
            }
        }
        return 0;
    }
    let mut failed = 0;
    for c in &cmds {
        if let Err(e) = hypr::run(c) {
            eprintln!("poste display : {e}");
            failed += 1;
        }
    }
    save_state(
        env,
        &State {
            profile: Some(p.name.clone()),
            sig: model::signature(mons),
            manual,
        },
    );
    println!(
        "Profil {} appliqué ({} commande(s){}).",
        p.label(),
        cmds.len(),
        if failed > 0 {
            format!(", {failed} refusée(s)")
        } else {
            String::new()
        }
    );
    failed
}

/// « auto » : choisit selon les écrans. `force` : réappliquer même si rien n'a changé (après un
/// rechargement de la config d'Hyprland, qui remet les règles du fichier). `notify` : prévenir si
/// le profil change. Renvoie 0 si tout va bien.
fn auto(env: &Env, cfg: &Config, dry_run: bool, force: bool, notify: bool) -> i32 {
    let mons = match monitors() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("poste display : {e}");
            return 1;
        }
    };
    let sig = model::signature(&mons);
    let st = load_state(env);
    let same_screens = st.as_ref().is_some_and(|s| s.sig == sig);
    if same_screens && !force {
        if dry_run {
            println!("Les écrans n'ont pas changé depuis le dernier profil : rien à faire.");
        }
        return 0;
    }
    // Après un rechargement, on remet ce qui était appliqué (y compris un profil choisi à la main).
    let chosen: Option<&Profile> = match (&st, same_screens && force) {
        (Some(s), true) if s.manual => s.profile.as_deref().and_then(|n| cfg.find(n)),
        _ => model::choose(cfg, &mons),
    };
    let Some(p) = chosen else {
        if dry_run {
            println!("Aucun profil ne correspond aux écrans branchés : rien n'est changé.");
        } else {
            save_state(
                env,
                &State {
                    profile: None,
                    sig,
                    manual: false,
                },
            );
        }
        return 0;
    };
    let changed = st.as_ref().and_then(|s| s.profile.as_deref()) != Some(p.name.as_str());
    let failed = apply_profile(env, cfg, p, &mons, dry_run, false);
    if !dry_run && notify && cfg.notify && changed {
        let n = model::externals(cfg, &mons).len();
        notify::send(
            &format!("Écrans : profil {}", p.label()),
            &format!("{n} écran(s) externe(s) branché(s)."),
        );
    }
    i32::from(failed > 0)
}

// ---------------------------------------------------------------- commandes

fn cmd_list(env: &Env, cfg: &Config) -> i32 {
    if cfg.profile.is_empty() {
        println!(
            "Aucun profil. Copiez display.toml.example du module vers {} puis ajustez-le.",
            env.display(&Config::file(env))
        );
        return 0;
    }
    for p in &cfg.profile {
        let det = if p.detect.is_empty() && p.detect_any.is_empty() {
            "aucun écran externe".to_string()
        } else {
            let mut v = Vec::new();
            if !p.detect.is_empty() {
                v.push(format!("tous : {}", p.detect.join(", ")));
            }
            if !p.detect_any.is_empty() {
                v.push(format!("au moins un : {}", p.detect_any.join(", ")));
            }
            v.join(" ; ")
        };
        println!("{:<10} {}  [{det}]", p.name, p.label());
    }
    0
}

fn cmd_status(env: &Env, cfg: &Config) -> i32 {
    let mons = match monitors() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("poste display : {e}");
            return 1;
        }
    };
    println!("Écrans (interne : {})", cfg.internal);
    for m in &mons {
        println!(
            "  {:<10} {}{}",
            m.name,
            m.description,
            if m.disabled { "  [désactivé]" } else { "" }
        );
    }
    match load_state(env) {
        Some(s) => println!(
            "Profil courant : {}{}",
            s.profile.as_deref().unwrap_or("(aucun ne correspondait)"),
            if s.manual {
                "  (choisi à la main)"
            } else {
                ""
            }
        ),
        None => println!("Profil courant : inconnu (rien appliqué depuis le démarrage)"),
    }
    match model::choose(cfg, &mons) {
        Some(p) => println!("« auto » choisirait : {}", p.name),
        None => println!("« auto » ne choisirait aucun profil (aucun ne correspond)"),
    }
    0
}

/// Entrées du lanceur : un profil par ligne, plus « automatique ».
pub fn items_json(cfg: &Config) -> String {
    let cmd = |args: &str| serde_json::json!({"type": "command", "command": format!("poste display {args}")});
    let mut items: Vec<serde_json::Value> = cfg
        .profile
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": format!("profile:{}", p.name),
                "name": format!("Écrans : {}", p.label()),
                "subtitle": format!("Appliquer le profil « {} »", p.label()),
                "icon": "\u{f0379}",
                "verb": "Appliquer",
                "keywords": ["écrans", "ecrans", "display", "profil", p.name],
                "action": cmd(&format!("apply '{}'", p.name)),
            })
        })
        .collect();
    if !cfg.profile.is_empty() {
        items.push(serde_json::json!({
            "id": "auto",
            "name": "Écrans : automatique",
            "subtitle": "Choisir le profil selon les écrans branchés",
            "icon": "\u{f0379}",
            "verb": "Appliquer",
            "keywords": ["écrans", "ecrans", "display", "auto", "automatique"],
            "action": cmd("auto"),
        }));
    }
    serde_json::Value::Array(items).to_string()
}

// ---------------------------------------------------------------- démon

fn watch(env: &Env) -> i32 {
    let load = || match Config::load(env) {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("poste display : {e:#}");
            None
        }
    };
    if let Some(cfg) = load() {
        auto(env, &cfg, false, false, true);
    }
    let mut last_apply = Instant::now();
    loop {
        let Some(path) = hypr::event_socket() else {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        };
        let Ok(stream) = UnixStream::connect(&path) else {
            std::thread::sleep(Duration::from_secs(2));
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        // (échéance, forcer) : un événement de branchement attend la stabilisation d'Hyprland.
        let mut pending: Option<(Instant, bool)> = None;
        loop {
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) if line.ends_with('\n') => {
                    let debounce = Duration::from_millis(
                        load().map_or(Config::default().debounce_ms, |c| c.debounce_ms),
                    );
                    if line.starts_with("monitoradded") || line.starts_with("monitorremoved") {
                        let force = pending.is_some_and(|(_, f)| f);
                        pending = Some((Instant::now() + debounce, force));
                    } else if line.starts_with("configreloaded")
                        && last_apply.elapsed() > Duration::from_secs(3)
                        && pending.is_none()
                    {
                        pending = Some((Instant::now() + debounce, true));
                    }
                    line.clear();
                }
                Ok(_) => {}
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) => {}
                Err(_) => break,
            }
            if let Some((due, force)) = pending {
                if Instant::now() >= due {
                    pending = None;
                    if let Some(cfg) = load() {
                        auto(env, &cfg, false, force, true);
                    }
                    last_apply = Instant::now();
                }
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

pub fn run(argv: &[String]) -> i32 {
    let Some(cmd) = argv.first() else {
        println!("{USAGE}");
        return 2;
    };
    let rest = &argv[1..];
    let env = match Env::from_process() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("poste display : {e:#}");
            return 1;
        }
    };
    let cfg = match Config::load(&env) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("poste display : {e:#}");
            return 1;
        }
    };
    let dry_run = rest.iter().any(|a| a == "--dry-run" || a == "-n");
    match cmd.as_str() {
        "status" => cmd_status(&env, &cfg),
        "list" => cmd_list(&env, &cfg),
        "items" => {
            println!("{}", items_json(&cfg));
            0
        }
        "auto" => auto(&env, &cfg, dry_run, false, true),
        "apply" => {
            let Some(name) = rest.iter().find(|a| !a.starts_with('-')) else {
                eprintln!("poste display : apply demande un nom de profil (poste display list)");
                return 2;
            };
            let Some(p) = cfg.find(name) else {
                eprintln!("poste display : profil « {name} » inconnu (poste display list)");
                return 1;
            };
            match monitors() {
                Ok(mons) => i32::from(apply_profile(&env, &cfg, p, &mons, dry_run, true) > 0),
                Err(e) => {
                    eprintln!("poste display : {e}");
                    1
                }
            }
        }
        "watch" => watch(&env),
        _ => {
            println!("{USAGE}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_du_lanceur() {
        let cfg = Config::parse(include_str!("../display.toml.example")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&items_json(&cfg)).unwrap();
        let a = v.as_array().unwrap();
        assert_eq!(a.len(), 4);
        assert_eq!(a[0]["action"]["command"], "poste display apply 'nomade'");
        assert_eq!(a[3]["action"]["command"], "poste display auto");
        assert_eq!(items_json(&Config::default()), "[]");
    }

    #[test]
    fn etat_aller_retour() {
        let st = State {
            profile: Some("a".into()),
            sig: "s".into(),
            manual: true,
        };
        let j = serde_json::to_string(&st).unwrap();
        assert_eq!(serde_json::from_str::<State>(&j).unwrap(), st);
    }
}
