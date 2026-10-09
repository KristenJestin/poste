//! `poste record items` : la liste de l'extension « Record » du lanceur de l'île.

use crate::env::{load_config, Env};
use crate::hypr::{clients, mapped, monitor_rect, monitors, text, window_rect, Obj, Rect};
use crate::pyval::{basename, dirname, head_chars, py_str, truthy, truthy_opt};
use crate::state::{public, read_state};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize, PartialEq)]
pub struct Action {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub command: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct SubAction {
    pub label: String,
    pub icon: String,
    pub action: Action,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Item {
    pub id: String,
    pub name: Value,
    pub subtitle: String,
    pub icon: Value,
    pub verb: String,
    pub keywords: Vec<Value>,
    pub action: Action,
    pub actions: Vec<SubAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<Rect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub badge: Option<String>,
}

/// `poste record 'a' 'b'` : arguments entre apostrophes pour le shell.
pub fn cmd_of(args: &[&str]) -> String {
    let mut parts = vec!["poste".to_string(), "record".to_string()];
    parts.extend(
        args.iter()
            .map(|a| format!("'{}'", a.replace('\'', "'\\''"))),
    );
    parts.join(" ")
}

fn sub(label: &str, icon: &str, command: String) -> SubAction {
    SubAction {
        label: label.into(),
        icon: icon.into(),
        action: Action {
            kind: "command",
            command,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn item(
    id: String,
    name: Value,
    subtitle: String,
    icon: Value,
    command: String,
    verb: &str,
    keywords: Vec<Value>,
    actions: Vec<SubAction>,
    highlight: Option<Rect>,
) -> Item {
    Item {
        id,
        name,
        subtitle,
        icon,
        verb: verb.into(),
        keywords,
        action: Action {
            kind: "command",
            command,
        },
        actions,
        highlight,
        badge: None,
    }
}

fn kw(words: &[&str]) -> Vec<Value> {
    words.iter().map(|w| Value::from(*w)).collect()
}

/// Les actions Ctrl+B d'une cible : sans son, bureau, bureau + micro, dernier, dossier.
pub fn audio_actions(target: &[&str], preset: Option<&str>) -> Vec<SubAction> {
    let with = |audio: &str| {
        let mut a = vec!["start"];
        a.extend_from_slice(target);
        a.extend(["--audio", audio]);
        cmd_of(&a)
    };
    let mut last = vec!["open-last"];
    let mut dir = vec!["open-dir"];
    if let Some(p) = preset.filter(|p| !p.is_empty()) {
        last.extend(["--preset", p]);
        dir.extend(["--preset", p]);
    }
    vec![
        sub("No audio", "\u{f075f}", with("none")),
        sub("With desktop audio", "\u{f057e}", with("desktop")),
        sub(
            "With desktop + microphone",
            "\u{f036c}",
            with("desktop+mic"),
        ),
        sub("Play last recording", "\u{f040a}", cmd_of(&last)),
        sub("Open recordings folder", "\u{f024b}", cmd_of(&dir)),
    ]
}

/// Classe de fenêtre -> icône, d'après les fichiers .desktop (StartupWMClass ou nom).
pub fn desktop_icons(env: &Env) -> HashMap<String, String> {
    let mut icons = HashMap::new();
    let dirs = [
        format!("{}/.local/share/applications", env.home),
        "/usr/share/applications".to_string(),
    ];
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for entry in rd.flatten() {
            let n = entry.file_name().to_string_lossy().into_owned();
            let Some(stem) = n.strip_suffix(".desktop") else {
                continue;
            };
            let Ok(bytes) = std::fs::read(entry.path()) else {
                continue;
            };
            let content = String::from_utf8_lossy(&bytes);
            let (mut icon, mut wm) = (None, None);
            for line in content.lines() {
                if line.starts_with('[') && !line.starts_with("[Desktop Entry]") {
                    break;
                }
                if let Some(v) = line.strip_prefix("Icon=") {
                    icon = Some(v.trim().to_string());
                } else if let Some(v) = line.strip_prefix("StartupWMClass=") {
                    wm = Some(v.trim().to_string());
                }
            }
            if let Some(icon) = icon.filter(|i| !i.is_empty()) {
                for key in [wm.as_deref(), Some(stem)].into_iter().flatten() {
                    if !key.is_empty() {
                        icons
                            .entry(key.to_lowercase())
                            .or_insert_with(|| icon.clone());
                    }
                }
            }
        }
    }
    icons
}

/// Les deux entrées affichées pendant un enregistrement : Stop et Pause / Resume.
pub fn items_recording(label: &str, file: &str, paused: bool) -> Vec<Item> {
    let quoted = dirname(file).replace('\'', "'\\''");
    vec![
        item(
            "stop".into(),
            "Stop recording".into(),
            format!("{} · {}", label, basename(file)),
            "\u{f04db}".into(),
            cmd_of(&["stop"]),
            "Stop",
            kw(&["stop", "arrêter"]),
            vec![sub(
                "Open recordings folder",
                "\u{f024b}",
                format!("xdg-open '{quoted}'"),
            )],
            None,
        ),
        item(
            "pause".into(),
            if paused {
                "Resume recording"
            } else {
                "Pause recording"
            }
            .into(),
            label.to_string(),
            if paused { "\u{f040a}" } else { "\u{f03e4}" }.into(),
            cmd_of(&["toggle"]),
            if paused { "Resume" } else { "Pause" },
            kw(&["pause", "resume", "reprendre"]),
            vec![],
            None,
        ),
    ]
}

/// Les entrées au repos : préréglages, fenêtres, écrans, zone.
pub fn items_idle(
    presets: &[Obj],
    wins: &[Obj],
    mons: &[Obj],
    icons: &HashMap<String, String>,
) -> Vec<Item> {
    let mut out = Vec::new();
    for p in presets {
        let Some(Value::String(id)) = p.get("id") else {
            continue;
        };
        let target = ["--preset", id.as_str()];
        let hl = if truthy_opt(p.get("window")) {
            crate::hypr::find_window(&p["window"], wins).and_then(|w| window_rect(&w, mons))
        } else if truthy_opt(p.get("monitor")) {
            let want = &p["monitor"];
            mons.iter()
                .find(|m| crate::pyval::py_eq(m.get("name"), Some(want)))
                .map(monitor_rect)
        } else {
            None
        };
        let subtitle = match p.get("subtitle") {
            Some(s) if truthy(s) => format!("Preset · {}", py_str(s)),
            _ => "Preset".to_string(),
        };
        let mut keywords = kw(&["preset"]);
        match p.get("keywords") {
            Some(Value::Array(a)) => keywords.extend(a.iter().cloned()),
            Some(Value::String(s)) => {
                keywords.extend(s.chars().map(|c| Value::from(c.to_string())))
            }
            _ => {}
        }
        let mut it = item(
            format!("preset:{id}"),
            p.get("name")
                .cloned()
                .unwrap_or_else(|| Value::from(id.as_str())),
            subtitle,
            p.get("icon")
                .cloned()
                .unwrap_or_else(|| Value::from("\u{f044b}")),
            cmd_of(&["start", "--preset", id]),
            "Record",
            keywords,
            audio_actions(&target, Some(id)),
            hl,
        );
        it.badge = Some("\u{f04ce}".into());
        out.push(it);
    }
    for c in wins {
        if !mapped(c) || !truthy_opt(c.get("title")) {
            continue;
        }
        let Some(Value::String(addr)) = c.get("address") else {
            continue;
        };
        let cls = text(c, "class");
        let lower = cls.to_lowercase();
        let icon = icons
            .get(&lower)
            .or_else(|| icons.get(lower.rsplit('.').next().unwrap_or("")))
            .cloned()
            .unwrap_or_else(|| "\u{f05af}".into());
        out.push(item(
            format!("window:{addr}"),
            Value::from(head_chars(&text(c, "title"), 80)),
            format!("Window · {cls} · with its own audio"),
            Value::from(icon),
            cmd_of(&["start", "--window", addr]),
            "Record",
            vec![
                Value::from("window"),
                Value::from("fenêtre"),
                Value::from(text(c, "class")),
            ],
            audio_actions(&["--window", addr], None),
            window_rect(c, mons),
        ));
    }
    for m in mons {
        let Some(Value::String(name)) = m.get("name") else {
            continue;
        };
        let dim = |k: &str| crate::pyval::py_str_opt(m.get(k), "None");
        let desc = match m.get("description") {
            Some(Value::String(s)) => head_chars(s, 40),
            Some(other) => head_chars(&py_str(other), 40),
            None => String::new(),
        };
        out.push(item(
            format!("monitor:{name}"),
            Value::from(format!("Screen {name}")),
            format!(
                "{}x{} · {} · with desktop audio",
                dim("width"),
                dim("height"),
                desc
            ),
            Value::from("\u{f0379}"),
            cmd_of(&["start", "--monitor", name]),
            "Record",
            kw(&["screen", "écran", "monitor"]),
            audio_actions(&["--monitor", name], None),
            Some(monitor_rect(m)),
        ));
    }
    out.push(item(
        "region".into(),
        "Region…".into(),
        "Select an area · with desktop audio".into(),
        "\u{f0a6d}".into(),
        cmd_of(&["start", "--region"]),
        "Record",
        kw(&["region", "zone", "area"]),
        audio_actions(&["--region"], None),
        None,
    ));
    out
}

/// `items()` : la liste complète en JSON (une ligne), à afficher.
pub fn items_json(env: &Env) -> String {
    let st = read_state(env);
    let out = if let Some(st) = st {
        let p = public(Some(&st));
        items_recording(
            &p.label.unwrap_or_default(),
            &p.file.unwrap_or_default(),
            p.paused.unwrap_or(false),
        )
    } else {
        let (_, presets) = load_config(env);
        let mons = monitors();
        let icons = desktop_icons(env);
        let wins = clients();
        items_idle(&presets, &wins, &mons, &icons)
    };
    crate::pyval::dumps(&out)
}
