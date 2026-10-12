//! Génération des commandes d'un profil. Chaque commande a une forme Lua (`hyprctl eval`, Hyprland
//! 0.56 avec config Lua) et, quand elle existe, une forme historique (`hyprctl keyword|dispatch`)
//! essayée si la première échoue. Rien ici n'exécute quoi que ce soit.

use crate::config::{Config, Internal, MonitorCfg, Profile, RescueFrom, Scale};
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

#[derive(Debug, Deserialize)]
struct Client {
    address: String,
    workspace: ClientWs,
}
#[derive(Debug, Deserialize)]
struct ClientWs {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    name: String,
}

/// Fenêtres (adresses) à rapatrier selon `hyprctl clients -j`.
fn rescue_cmds(p: &Profile, clients_json: &str) -> Vec<Cmd> {
    let clients: Vec<Client> = serde_json::from_str(clients_json).unwrap_or_default();
    let mut out = Vec::new();
    for r in &p.rescue {
        // Règles vérifiées au chargement de display.toml.
        for (from, to) in r.pairs().unwrap_or_default() {
            let hit = |c: &&Client| match &from {
                RescueFrom::Id(n) => c.workspace.id == *n,
                RescueFrom::Name(n) => c.workspace.name == *n,
            };
            for c in clients.iter().filter(hit) {
                if c.workspace.id.to_string() == to {
                    continue;
                }
                let label = match &from {
                    RescueFrom::Id(n) => n.to_string(),
                    RescueFrom::Name(n) => format!("« {n} »"),
                };
                out.push(Cmd {
                    desc: format!("fenêtre {} : de {label} vers {to}", c.address),
                    lua: format!(
                        "hl.dispatch(hl.dsp.window.move({{ workspace = {}, follow = false, window = {} }}))",
                        q(&to),
                        q(&format!("address:{}", c.address))
                    ),
                    legacy: Some(vec![
                        "dispatch".into(),
                        "movetoworkspacesilent".into(),
                        format!("{to},address:{}", c.address),
                    ]),
                });
            }
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

/// Toutes les commandes d'un profil, dans l'ordre : fenêtres à rapatrier, puis écrans (l'interne
/// d'abord s'il reste actif, après les autres s'il est désactivé : jamais zéro écran). Les espaces
/// de travail de chaque écran ne sont pas touchés : ils viennent de `monitors.lua`.
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::parse_monitors;
    use crate::model::tests::{fake, AOC_LEFT, AOC_MID, EDP};

    fn cfg() -> Config {
        Config::parse(include_str!("../display.toml.example")).unwrap()
    }

    #[test]
    fn boulot_avec_les_deux_ecrans() {
        let c = cfg();
        let ms = parse_monitors(&fake(&[EDP, AOC_MID, AOC_LEFT])).unwrap();
        let cmds = commands(&c, c.find("boulot").unwrap(), &ms, "[]");
        let lua: Vec<&str> = cmds.iter().map(|c| c.lua.as_str()).collect();
        assert_eq!(
            lua,
            [
                "hl.monitor({ output = \"eDP-1\", mode = \"2880x1800@120\", position = \"0x0\", scale = 1.6 })",
                "hl.monitor({ output = \"HDMI-A-1\", mode = \"1920x1080@144\", position = \"0x-1080\", scale = 1 })",
                "hl.monitor({ output = \"DP-1\", mode = \"1920x1080@144\", position = \"-1080x-1080\", scale = 1, transform = 1 })",
            ]
        );
        // Forme historique de repli.
        assert_eq!(
            cmds[0].legacy.as_deref().unwrap(),
            ["keyword", "monitor", "eDP-1,2880x1800@120,0x0,1.6"]
        );
        assert_eq!(
            cmds[2].legacy.as_deref().unwrap()[2],
            "DP-1,1920x1080@144,-1080x-1080,1,transform,1"
        );
    }

    #[test]
    fn ecran_absent_ignore() {
        let c = cfg();
        let ms = parse_monitors(&fake(&[EDP, AOC_MID])).unwrap();
        let cmds = commands(&c, c.find("boulot").unwrap(), &ms, "[]");
        assert_eq!(cmds.len(), 2);
        assert!(!cmds.iter().any(|c| c.lua.contains("\"DP-1\"")));
    }

    #[test]
    fn nomade_range_les_fenetres_sur_le_portable() {
        let c = cfg();
        let ms = parse_monitors(&fake(&[EDP])).unwrap();
        let clients = r#"[{"address":"0xaa","workspace":{"id":1,"name":"1"}},
                          {"address":"0xbb","workspace":{"id":13,"name":"3"}},
                          {"address":"0xcc","workspace":{"id":22,"name":"2"}},
                          {"address":"0xdd","workspace":{"id":-99,"name":"mid"}}]"#;
        let cmds = commands(&c, c.find("nomade").unwrap(), &ms, clients);
        assert_eq!(cmds.len(), 3, "{cmds:?}");
        assert_eq!(cmds[0].lua, "hl.dispatch(hl.dsp.window.move({ workspace = \"21\", follow = false, window = \"address:0xaa\" }))");
        assert_eq!(
            cmds[1].legacy.as_deref().unwrap(),
            ["dispatch", "movetoworkspacesilent", "23,address:0xbb"]
        );
        assert!(cmds[2].lua.starts_with("hl.monitor({ output = \"eDP-1\""));
    }

    #[test]
    fn rapatrier_par_nom() {
        let c =
            Config::parse("[[profile]]\nname=\"a\"\n[[profile.rescue]]\nfrom=\"mid\"\nto=\"11\"\n")
                .unwrap();
        let clients = r#"[{"address":"0xaa","workspace":{"id":-1337,"name":"mid"}},
                          {"address":"0xbb","workspace":{"id":11,"name":"1"}}]"#;
        let cmds = commands(&c, &c.profile[0], &[], clients);
        assert_eq!(cmds[0].desc, "fenêtre 0xaa : de « mid » vers 11");
        assert_eq!(cmds.len(), 2, "une fenêtre rangée, puis l'écran interne");
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
    fn guillemets_echappes() {
        assert_eq!(q("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
