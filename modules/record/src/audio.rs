//! Sound: gpu-screen-recorder sources, window application, file name.

use crate::hypr::{text, Obj};
use crate::proc::{capture, prog, GSR};
use std::time::Duration;

/// Names of the apps that play sound (`gpu-screen-recorder --list-application-audio`).
fn app_audio_names() -> Vec<String> {
    capture(
        &prog(GSR),
        &["--list-application-audio"],
        Duration::from_secs(5),
    )
    .map(|out| {
        out.lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    })
    .unwrap_or_default()
}

/// Choice of the PipeWire name from a list: the class, the initial class or the first word of the title.
pub fn match_app_audio(client: &Obj, names: &[String]) -> Option<String> {
    let title = text(client, "title");
    let first = title.split(' ').next().unwrap_or("").to_string();
    let keys = [text(client, "class"), text(client, "initialClass"), first];
    for k in keys.iter().filter(|k| !k.is_empty()) {
        let kl = k.to_lowercase();
        for n in names {
            let nl = n.to_lowercase();
            if nl == kl || kl.contains(&nl) || nl.contains(&kl) {
                return Some(n.clone());
            }
        }
    }
    None
}

/// The PipeWire name of a window's app ("Zen", "Unity"…), if it plays sound.
pub fn app_audio_for(client: &Obj) -> Option<String> {
    match_app_audio(client, &app_audio_names())
}

/// `audio_sources(audio, app)`: the `-a` argument of gpu-screen-recorder (empty: no sound).
pub fn audio_sources(audio: &str, app: Option<&str>) -> String {
    let audio = if audio.is_empty() { "none" } else { audio };
    if audio.contains('|')
        || audio.contains(':')
        || audio.starts_with("default_")
        || audio.starts_with("alsa_")
    {
        return audio.to_string();
    }
    let mut parts = Vec::new();
    for word in audio.split('+') {
        match word {
            "desktop" => parts.push("default_output".to_string()),
            "mic" => parts.push("default_input".to_string()),
            "app" => match app {
                Some(a) if !a.is_empty() => parts.push(format!("app:{a}")),
                _ => parts.push("default_output".to_string()),
            },
            _ => {}
        }
    }
    parts.join("|")
}

/// Safe file name (60 characters at most), "Recording" if nothing remains.
pub fn slug(text: &str) -> String {
    let mut spaced = String::new();
    let mut in_run = false;
    for ch in text.chars() {
        if "/\\:*?\"<>|".contains(ch) {
            if !in_run {
                spaced.push(' ');
            }
            in_run = true;
        } else {
            spaced.push(ch);
            in_run = false;
        }
    }
    let mut collapsed = String::new();
    let mut prev_space = false;
    for ch in spaced.trim().chars() {
        if ch.is_whitespace() {
            if !prev_space {
                collapsed.push(' ');
            }
            prev_space = true;
        } else {
            collapsed.push(ch);
            prev_space = false;
        }
    }
    let cut: String = collapsed.chars().take(60).collect();
    if cut.is_empty() {
        "Recording".into()
    } else {
        cut
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sources() {
        assert_eq!(audio_sources("none", None), "");
        assert_eq!(audio_sources("", None), "");
        assert_eq!(audio_sources("desktop", None), "default_output");
        assert_eq!(audio_sources("mic", None), "default_input");
        assert_eq!(
            audio_sources("desktop+mic", None),
            "default_output|default_input"
        );
        assert_eq!(audio_sources("app", Some("FMOD Ex App")), "app:FMOD Ex App");
        assert_eq!(audio_sources("app", None), "default_output");
        assert_eq!(audio_sources("app", Some("")), "default_output");
        assert_eq!(
            audio_sources("app+mic", Some("Zen")),
            "app:Zen|default_input"
        );
        assert_eq!(
            audio_sources("app:Unity|default_input", None),
            "app:Unity|default_input"
        );
        assert_eq!(audio_sources("default_output", Some("x")), "default_output");
        assert_eq!(audio_sources("alsa_input.x", None), "alsa_input.x");
        assert_eq!(audio_sources("unknown", None), "");
    }

    #[test]
    fn file_name() {
        assert_eq!(slug("A/B: C"), "A B C");
        assert_eq!(slug("  ///  "), "Recording");
        assert_eq!(slug(""), "Recording");
        assert_eq!(slug("a   b"), "a b");
        assert_eq!(slug(&"x".repeat(80)).chars().count(), 60);
    }

    #[test]
    fn window_app() {
        let c = json!({"class": "zen", "initialClass": "zen", "title": "Page - Zen"});
        let c = c.as_object().unwrap();
        let names = vec!["Firefox".to_string(), "Zen".to_string()];
        assert_eq!(match_app_audio(c, &names).as_deref(), Some("Zen"));
        assert_eq!(match_app_audio(c, &[]), None);
    }
}
