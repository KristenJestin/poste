//! Génération des commandes d'un profil. Chaque commande a une forme Lua (`hyprctl eval`, Hyprland
//! 0.56 avec config Lua) et, quand elle existe, une forme historique (`hyprctl keyword|dispatch`)
//! essayée si la première échoue. Rien ici n'exécute quoi que ce soit.

use crate::config::{Config, Internal, MonitorCfg, Profile, Scale};
use crate::model::{matches, Monitor};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Cmd {
    /// Description lisible (aperçu, messages d'erreur).
    pub desc: String,
    pub lua: String,
    /// Arguments de `hyprctl` pour la forme historique.
    pub legacy: Option<Vec<String>>,
}

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

struct Rule<'a> {
    output: &'a str,
    mode: Option<&'a str>,
    position: Option<&'a str>,
    scale: Option<&'a Scale>,
    transform: Option<u8>,
}

fn monitor_cmd(r: Rule) -> Cmd {
    let mode = r.mode.unwrap_or("preferred");
    let pos = r.position.unwrap_or("auto");
    let auto = Scale::Text("auto".into());
    let scale = r.scale.unwrap_or(&auto);
    let scale_lua = if scale.is_text() {
        q(&scale.text())
    } else {
        scale.text()
    };
    let mut lua = format!(
        "hl.monitor({{ output = {}, mode = {}, position = {}, scale = {scale_lua}",
        q(r.output),
        q(mode),
        q(pos)
    );
    let mut legacy = format!("{},{mode},{pos},{}", r.output, scale.text());
    if let Some(t) = r.transform {
        lua.push_str(&format!(", transform = {t}"));
        legacy.push_str(&format!(",transform,{t}"));
    }
    lua.push_str(" })");
    Cmd {
        desc: format!(
            "écran {} : {mode} à {pos}, échelle {}",
            r.output,
            scale.text()
        ),
        lua,
        legacy: Some(vec!["keyword".into(), "monitor".into(), legacy]),
    }
}

fn disable_cmd(output: &str) -> Cmd {
    Cmd {
        desc: format!("écran {output} : désactivé"),
        lua: format!("hl.monitor({{ output = {}, disabled = true }})", q(output)),
        legacy: Some(vec![
            "keyword".into(),
            "monitor".into(),
            format!("{output},disable"),
        ]),
    }
}

/// Nom de la sortie visée par un critère de profil (`internal`, EDID, `connector:`).
fn resolve<'a>(cfg: &'a Config, mons: &'a [Monitor], target: &str) -> Option<&'a str> {
    if target.trim() == "internal" {
        return Some(cfg.internal.as_str());
    }
    mons.iter()
        .filter(|m| m.name != cfg.internal)
        .find(|m| matches(target, m))
        .map(|m| m.name.as_str())
}

/// `1-10` en 1, 2, … 10 ; sinon l'identifiant tel quel.
pub fn expand_workspaces(spec: &str) -> Vec<String> {
    if let Some((a, b)) = spec.split_once('-') {
        if let (Ok(a), Ok(b)) = (a.trim().parse::<u32>(), b.trim().parse::<u32>()) {
            if a <= b && b - a < 100 {
                return (a..=b).map(|n| n.to_string()).collect();
            }
        }
    }
    vec![spec.trim().to_string()]
}

#[derive(Debug, Deserialize)]
struct Client {
    address: String,
    workspace: ClientWs,
}
#[derive(Debug, Deserialize)]
struct ClientWs {
    #[serde(default)]
    name: String,
}

/// Fenêtres (adresses) à rapatrier selon `hyprctl clients -j`.
fn rescue_cmds(p: &Profile, clients_json: &str) -> Vec<Cmd> {
    let clients: Vec<Client> = serde_json::from_str(clients_json).unwrap_or_default();
    let mut out = Vec::new();
    for r in &p.rescue {
        for c in clients.iter().filter(|c| c.workspace.name == r.from) {
            out.push(Cmd {
                desc: format!("fenêtre {} : de « {} » vers {}", c.address, r.from, r.to),
                lua: format!(
                    "hl.dispatch(hl.dsp.window.move({{ workspace = {}, follow = false, window = {} }}))",
                    q(&r.to),
                    q(&format!("address:{}", c.address))
                ),
                legacy: Some(vec![
                    "dispatch".into(),
                    "movetoworkspacesilent".into(),
                    format!("{},address:{}", r.to, c.address),
                ]),
            });
        }
    }
    out
}

fn internal_cmd(cfg: &Config, i: &Internal) -> Cmd {
    if !i.enabled {
        return disable_cmd(&cfg.internal);
    }
    monitor_cmd(Rule {
        output: &cfg.internal,
        mode: i.mode.as_deref(),
        position: i.position.as_deref(),
        scale: i.scale.as_ref(),
        transform: i.transform,
    })
}

fn external_cmd(output: &str, m: &MonitorCfg) -> Cmd {
    monitor_cmd(Rule {
        output,
        mode: m.mode.as_deref(),
        position: m.position.as_deref(),
        scale: m.scale.as_ref(),
        transform: m.transform,
    })
}

/// Toutes les commandes d'un profil, dans l'ordre : fenêtres à rapatrier, écrans (l'interne
/// d'abord s'il reste actif, après les autres s'il est désactivé : jamais zéro écran), règles
/// d'espaces de travail, puis déplacement immédiat des espaces existants.
pub fn commands(cfg: &Config, p: &Profile, mons: &[Monitor], clients_json: &str) -> Vec<Cmd> {
    let mut out = rescue_cmds(p, clients_json);
    let internal = p.internal.clone().unwrap_or_default();
    let internal_first = internal.enabled;
    if internal_first {
        out.push(internal_cmd(cfg, &internal));
    }
    for m in &p.monitor {
        if let Some(name) = resolve(cfg, mons, &m.matcher) {
            out.push(external_cmd(name, m));
        }
    }
    if !internal_first {
        out.push(internal_cmd(cfg, &internal));
    }
    let mut moves = Vec::new();
    for w in &p.workspace {
        let Some(mon) = resolve(cfg, mons, &w.monitor) else {
            continue;
        };
        for id in expand_workspaces(&w.workspace) {
            let mut lua = format!(
                "hl.workspace_rule({{ workspace = {}, monitor = {}",
                q(&id),
                q(mon)
            );
            let mut legacy = format!("{id},monitor:{mon}");
            if w.default {
                lua.push_str(", default = true");
                legacy.push_str(",default:true");
            }
            if w.persistent {
                lua.push_str(", persistent = true");
                legacy.push_str(",persistent:true");
            }
            lua.push_str(" })");
            out.push(Cmd {
                desc: format!("espace {id} : sur {mon}"),
                lua,
                legacy: Some(vec!["keyword".into(), "workspace".into(), legacy]),
            });
            moves.push(Cmd {
                desc: format!("espace {id} : déplacé vers {mon}"),
                lua: format!(
                    "hl.dispatch(hl.dsp.workspace.move({{ id = {}, monitor = {} }}))",
                    q(&id),
                    q(mon)
                ),
                legacy: Some(vec![
                    "dispatch".into(),
                    "moveworkspacetomonitor".into(),
                    format!("{id} {mon}"),
                ]),
            });
        }
    }
    out.extend(moves);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_monitors;
    use crate::model::tests::{fake, EDP, IIYAMA};

    fn cfg() -> Config {
        Config::parse(include_str!("../display.toml.example")).unwrap()
    }

    #[test]
    fn plages() {
        assert_eq!(expand_workspaces("1-3"), ["1", "2", "3"]);
        assert_eq!(expand_workspaces("name:mid"), ["name:mid"]);
        assert_eq!(expand_workspaces("5"), ["5"]);
    }

    #[test]
    fn boulot_avec_les_deux_ecrans() {
        let c = cfg();
        let ms = parse_monitors(&fake(&[EDP, ("HDMI-A-1", "Dell"), ("DP-1", "LG")])).unwrap();
        let cmds = commands(&c, c.find("boulot").unwrap(), &ms, "[]");
        let lua: Vec<&str> = cmds.iter().map(|c| c.lua.as_str()).collect();
        assert_eq!(
            lua[0],
            "hl.monitor({ output = \"eDP-1\", mode = \"2880x1800@120\", position = \"0x0\", scale = 1.6 })"
        );
        assert_eq!(
            lua[1],
            "hl.monitor({ output = \"HDMI-A-1\", mode = \"1920x1080@144\", position = \"0x-1080\", scale = 1 })"
        );
        assert_eq!(
            lua[2],
            "hl.monitor({ output = \"DP-1\", mode = \"1920x1080@144\", position = \"-1080x-1080\", scale = 1, transform = 1 })"
        );
        assert!(lua.contains(&"hl.workspace_rule({ workspace = \"name:mid\", monitor = \"HDMI-A-1\", default = true, persistent = true })"));
        assert!(lua.contains(&"hl.workspace_rule({ workspace = \"7\", monitor = \"eDP-1\" })"));
        assert!(lua.contains(
            &"hl.dispatch(hl.dsp.workspace.move({ id = \"name:left\", monitor = \"DP-1\" }))"
        ));
        // Forme historique de repli.
        assert_eq!(
            cmds[0].legacy.as_deref().unwrap(),
            ["keyword", "monitor", "eDP-1,2880x1800@120,0x0,1.6"]
        );
        assert_eq!(
            cmds[2].legacy.as_deref().unwrap()[2],
            "DP-1,1920x1080@144,-1080x-1080,1,transform,1"
        );
        // Les règles d'espaces précèdent les déplacements.
        let last_rule = lua
            .iter()
            .rposition(|l| l.starts_with("hl.workspace_rule"))
            .unwrap();
        let first_move = lua
            .iter()
            .position(|l| l.contains("workspace.move"))
            .unwrap();
        assert!(last_rule < first_move);
    }

    #[test]
    fn ecran_absent_ignore() {
        let c = cfg();
        let ms = parse_monitors(&fake(&[EDP, ("HDMI-A-1", "Dell")])).unwrap();
        let cmds = commands(&c, c.find("boulot").unwrap(), &ms, "[]");
        assert!(!cmds.iter().any(|c| c.lua.contains("\"DP-1\"")));
        assert!(!cmds.iter().any(|c| c.lua.contains("name:left")));
    }

    #[test]
    fn nomade_rapatrie_les_fenetres() {
        let c = cfg();
        let ms = parse_monitors(&fake(&[EDP])).unwrap();
        let clients = r#"[{"address":"0xaa","workspace":{"id":-99,"name":"mid"}},
                          {"address":"0xbb","workspace":{"id":3,"name":"3"}},
                          {"address":"0xcc","workspace":{"id":-98,"name":"left"}}]"#;
        let cmds = commands(&c, c.find("nomade").unwrap(), &ms, clients);
        assert_eq!(cmds[0].lua, "hl.dispatch(hl.dsp.window.move({ workspace = \"9\", follow = false, window = \"address:0xaa\" }))");
        assert_eq!(
            cmds[1].legacy.as_deref().unwrap(),
            ["dispatch", "movetoworkspacesilent", "10,address:0xcc"]
        );
        assert!(cmds[2].lua.starts_with("hl.monitor({ output = \"eDP-1\""));
        assert_eq!(
            cmds.iter()
                .filter(|c| c.lua.contains("movetoworkspace") || c.lua.contains("window.move"))
                .count(),
            2
        );
    }

    #[test]
    fn interne_desactive_en_dernier() {
        let c = Config::parse(
            "[[profile]]\nname=\"dock\"\ndetect=[\"X\"]\n[profile.internal]\nenabled=false\n[[profile.monitor]]\nmatch=\"X\"\nscale=\"auto\"\n",
        )
        .unwrap();
        let ms = parse_monitors(&fake(&[EDP, ("DP-5", "X")])).unwrap();
        let cmds = commands(&c, &c.profile[0], &ms, "[]");
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].lua, "hl.monitor({ output = \"DP-5\", mode = \"preferred\", position = \"auto\", scale = \"auto\" })");
        assert_eq!(
            cmds[1].lua,
            "hl.monitor({ output = \"eDP-1\", disabled = true })"
        );
        assert_eq!(cmds[1].legacy.as_deref().unwrap()[2], "eDP-1,disable");
    }

    #[test]
    fn maison_trois_ecrans_par_edid() {
        let c = cfg();
        let mut v = vec![EDP];
        v.extend(IIYAMA);
        let ms = parse_monitors(&fake(&v)).unwrap();
        let cmds = commands(&c, c.find("maison").unwrap(), &ms, "[]");
        assert!(cmds
            .iter()
            .any(|c| c.lua.contains("output = \"DP-3\"") && c.lua.contains("transform = 1")));
        assert!(cmds
            .iter()
            .any(|c| c.lua.contains("output = \"DP-2\"")
                && c.lua.contains("position = \"3000x420\"")));
    }

    #[test]
    fn guillemets_echappes() {
        assert_eq!(q("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
