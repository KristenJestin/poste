//! Calculs purs (son, noms de fichier, rectangles, recherche de fenêtre, commandes du lanceur)
//! comparés, sur les mêmes entrées, à ceux du script Python d'origine.

mod common;

use common::*;
use poste_record::audio::{audio_sources, match_app_audio, slug};
use poste_record::hypr::{find_window, monitor_rect, window_rect, Obj};
use poste_record::items::cmd_of;
use serde_json::{json, Value};
use std::io::Write;
use std::process::Stdio;

fn objs(v: &Value) -> Vec<Obj> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|o| o.as_object().unwrap().clone())
        .collect()
}

fn python_calc(input: &Value) -> Value {
    let mut child = std::process::Command::new("python3")
        .arg(manifest_dir().join("tests/py_calc.py"))
        .arg(manifest_dir().join("poste-record.py"))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn calculs_identiques_au_script_python() {
    if !python_available() {
        eprintln!("python3 absent : test ignoré");
        return;
    }
    let mut audio = Vec::new();
    for a in [
        "none",
        "",
        "app",
        "desktop",
        "mic",
        "desktop+mic",
        "app+mic",
        "mic+app+desktop",
        "app+app",
        "desktop+",
        "+",
        "bruit",
        "app:Unity|default_input",
        "default_output",
        "default_input+x",
        "alsa_output.pci",
        "a|b",
        "x:y",
        "Desktop",
        "mic+inconnu",
    ] {
        for app in [json!(null), json!(""), json!("FMOD Ex App"), json!("Zen")] {
            audio.push(json!([a, app]));
        }
    }
    let slugs = [
        "Unity",
        "",
        "   ",
        "A/B",
        "a:b*c?d\"e<f>g|h",
        "  espaces   multiples  ",
        "///",
        "tab\tet\nretour",
        "é".repeat(70).as_str(),
        &format!("{} fin", "x".repeat(59)),
        "Screen DP-1",
        "a\u{a0}b",
        "Preset · é",
    ]
    .map(|s| s.to_string());
    let monitors = json!([
        {"name": "A", "x": 0, "y": 0, "width": 3840, "height": 2160, "scale": 1.5, "transform": 0},
        {"name": "B", "x": 10, "y": -5, "width": 1920, "height": 1080, "scale": 1.0, "transform": 1},
        {"name": "C", "x": 0, "y": 0, "width": 1001, "height": 5, "scale": 2.0, "transform": 3},
        {"name": "D", "width": 3, "height": 5, "scale": 2, "transform": 0},
        {"name": "E", "width": 5, "height": 3, "scale": 2, "transform": 4},
        {"name": "F", "width": 2560, "height": 1440, "scale": 1.25, "transform": 2},
        {"name": "G", "width": 1366, "height": 768, "scale": 1.0, "transform": 5},
        {"name": "H", "scale": 0},
        {"name": "I", "width": 2880, "height": 1800, "scale": 1.6, "transform": 0},
        {"name": "J", "width": 1, "height": 1, "scale": 2.0, "transform": 0},
        {"name": "K", "width": 7, "height": 9, "scale": 2.0, "transform": 0},
    ]);
    let mons_for_windows = json!([
        {"activeWorkspace": {"id": 1}, "specialWorkspace": {"id": 0}},
        {"activeWorkspace": {"id": 4}, "specialWorkspace": {"id": -98}},
        {"specialWorkspace": null}
    ]);
    let clients = json!([
        {"address": "a", "workspace": {"id": 1}, "at": [1, 2], "size": [3, 4], "class": "Unity", "title": "Un", "mapped": true},
        {"address": "b", "workspace": {"id": 4}, "at": [5, 6], "size": [7, 8], "class": "unity", "title": "Deux"},
        {"address": "c", "workspace": {"id": 7}, "at": [0, 0], "size": [1, 1], "class": "Other"},
        {"address": "d", "workspace": {"id": -98}, "at": [0, 0], "size": [100, 50], "class": "zen", "title": "Zen x", "mapped": false},
        {"address": "e", "workspace": {"id": 0}, "at": [0, 0], "size": [20, 20], "class": "x"},
        {"address": "f", "at": [0, 0], "size": [10, 10], "class": "no-ws"},
        {"address": "g", "workspace": {}, "size": [30, 3], "class": "empty-ws"},
        {"address": "h", "workspace": {"id": 1.0}, "at": [1.5, 2], "size": [3, 4], "class": "float"}
    ]);
    let matches = json!([
        {"class": "^Unity$"}, {"class": "unity"}, {"title": "deux"}, {"class": "^Unity$", "title": "^Un$"},
        {"class": "nope"}, {}, {"class": ""}, {"class": "x|zen"}, {"title": "(?i)ZEN"}
    ]);
    let app_audio = json!([
        [{"class": "zen", "initialClass": "zen", "title": "Page - Zen"}, ["Firefox", "Zen"]],
        [{"class": "Unity", "title": "Project - Unity"}, ["FMOD Ex App"]],
        [{"class": "", "initialClass": "", "title": "Spotify Premium"}, ["Spotify"]],
        [{"class": "kitty", "title": "x"}, []],
        [{"class": "firefox-esr", "title": "x"}, ["Firefox"]],
        [{"title": "Unity Hub"}, ["unity"]]
    ]);
    let cmds = json!([
        ["stop"],
        ["start", "--preset", "it's"],
        ["start", "--monitor", "DP-1", "--audio", "desktop+mic"],
        ["open-last", "--preset", "unity"],
        ["x", "a b", "c\"d", "'"]
    ]);

    let input = json!({
        "audio": audio, "slug": slugs, "monitors": monitors, "clients": clients, "matches": matches,
        "app_audio": app_audio, "cmd_of": cmds
    });
    // window_rect utilise ses propres écrans : on les passe à Python par la même clé.
    let mut py_input = input.clone();
    py_input["monitors"] = mons_for_windows.clone();
    let py_windows = python_calc(&py_input)["window_rect"].clone();
    let py = python_calc(&input);

    // Son
    let rs: Vec<Value> = audio
        .iter()
        .map(|a| json!(audio_sources(a[0].as_str().unwrap(), a[1].as_str())))
        .collect();
    assert_eq!(py["audio"], json!(rs));
    // Noms de fichier
    let rs: Vec<Value> = input["slug"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| json!(slug(s.as_str().unwrap())))
        .collect();
    assert_eq!(py["slug"], json!(rs));
    // Rectangles d'écran
    let rs: Vec<Value> = objs(&monitors)
        .iter()
        .map(|m| serde_json::to_value(monitor_rect(m)).unwrap())
        .collect();
    assert_eq!(py["monitor_rect"], json!(rs));
    // Rectangles de fenêtre (avec les autres écrans)
    let mons = objs(&mons_for_windows);
    let rs: Vec<Value> = objs(&clients)
        .iter()
        .map(|c| {
            window_rect(c, &mons)
                .map(|r| serde_json::to_value(r).unwrap())
                .unwrap_or(Value::Null)
        })
        .collect();
    assert_eq!(py_windows, json!(rs));
    // Recherche de fenêtre
    let all = objs(&clients);
    let rs: Vec<Value> = matches
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            find_window(m, &all)
                .map(|w| w["address"].clone())
                .unwrap_or(Value::Null)
        })
        .collect();
    assert_eq!(py["find_window"], json!(rs));
    // Son de l'application d'une fenêtre
    let rs: Vec<Value> = app_audio
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let names: Vec<String> = p[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| n.as_str().unwrap().to_string())
                .collect();
            json!(match_app_audio(p[0].as_object().unwrap(), &names))
        })
        .collect();
    assert_eq!(py["app_audio"], json!(rs));
    // Commandes du lanceur
    let rs: Vec<Value> = cmds
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            let args: Vec<&str> = a
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap())
                .collect();
            json!(cmd_of(&args))
        })
        .collect();
    assert_eq!(py["cmd_of"], json!(rs));
}
