//! Results in the island launcher's format (contract: `docs/launcher-format.md`).

use crate::index::Hit;
use poste_core::Env;
use serde_json::{json, Value};
use std::path::Path;

const ICON_COPY_FILE: &str = "\u{f0191}";
const ICON_COPY: &str = "\u{f018f}";
const ICON_TERMINAL: &str = "\u{f018d}";
const ICON_REVEAL: &str = "\u{f0770}";

/// Theme icon for the type (freedesktop names, present in every common theme).
pub fn icon_for(path: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "folder";
    }
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "tif" | "tiff" | "heic"
        | "avif" | "ico" | "xcf" | "psd" | "kra" => "image-x-generic",
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v" | "wmv" => "video-x-generic",
        "mp3" | "flac" | "ogg" | "opus" | "wav" | "m4a" | "aac" => "audio-x-generic",
        "pdf" => "application-pdf",
        "zip" | "tar" | "gz" | "tgz" | "xz" | "zst" | "bz2" | "7z" | "rar" | "deb" | "rpm"
        | "iso" => "package-x-generic",
        "odt" | "doc" | "docx" | "rtf" | "epub" => "x-office-document",
        "ods" | "xls" | "xlsx" | "csv" | "tsv" => "x-office-spreadsheet",
        "odp" | "ppt" | "pptx" => "x-office-presentation",
        "rs" | "py" | "js" | "mjs" | "ts" | "tsx" | "jsx" | "sh" | "bash" | "zsh" | "fish"
        | "lua" | "c" | "h" | "cpp" | "hpp" | "cs" | "go" | "java" | "kt" | "rb" | "php"
        | "qml" | "html" | "css" | "scss" | "svelte" | "vue" | "json" | "toml" | "yaml" | "yml"
        | "xml" | "sql" | "gd" | "shader" => "text-x-script",
        _ => "text-x-generic",
    }
}

/// Quotes a word for the shell when it holds anything but safe characters.
pub fn shell_word(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:-+@".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// A launcher entry: Enter opens it (default application, file manager for a folder) and
/// remembers it in the recent files; four secondary actions.
pub fn to_json(env: &Env, hit: &Hit) -> Value {
    let p = Path::new(&hit.path);
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| hit.path.clone());
    let parent = p.parent().unwrap_or(p);
    // Terminal: in the folder itself, or in the file's folder.
    let term_dir = if hit.is_dir { p } else { parent };
    let quoted = shell_word(&hit.path);
    json!({
        "id": format!("file:{}", hit.path),
        "name": name,
        "subtitle": env.display(parent),
        "icon": icon_for(&hit.path, hit.is_dir),
        "verb": "Open",
        "action": {"type": "command", "command": format!("poste files open {quoted}")},
        "actions": [
            {"label": "Copy file", "icon": ICON_COPY_FILE,
             "action": {"type": "command", "command": format!("poste files copy {quoted}")}},
            {"label": "Copy path", "icon": ICON_COPY,
             "action": {"type": "copy", "text": hit.path}},
            {"label": "Open terminal here", "icon": ICON_TERMINAL,
             "action": {"type": "terminal", "cwd": term_dir.to_string_lossy(),
                        "command": "exec \"${SHELL:-bash}\""}},
            {"label": "Show in file manager", "icon": ICON_REVEAL,
             "action": {"type": "command", "command": format!("poste files reveal {quoted}")}},
        ],
    })
}

/// A lone entry without action (an error).
pub fn notice(id: &str, name: &str, subtitle: &str, icon: &str) -> Value {
    json!({"id": id, "name": name, "subtitle": subtitle, "icon": icon})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_and_actions() {
        let env = Env::with_home("/h");
        let v = to_json(
            &env,
            &Hit {
                path: "/h/Documents/l'été 2026.pdf".into(),
                is_dir: false,
            },
        );
        let quoted = "'/h/Documents/l'\\''été 2026.pdf'";
        assert_eq!(v["id"], "file:/h/Documents/l'été 2026.pdf");
        assert_eq!(v["name"], "l'été 2026.pdf");
        assert_eq!(v["subtitle"], "~/Documents");
        assert_eq!(v["icon"], "application-pdf");
        assert_eq!(v["action"]["type"], "command");
        assert_eq!(v["action"]["command"], format!("poste files open {quoted}"));
        let acts = v["actions"].as_array().unwrap();
        assert_eq!(acts.len(), 4);
        for a in acts {
            assert!(a["label"].is_string() && a["icon"].is_string());
        }
        assert_eq!(
            acts[0]["action"]["command"],
            format!("poste files copy {quoted}")
        );
        assert_eq!(acts[1]["action"]["type"], "copy");
        assert_eq!(acts[1]["action"]["text"], "/h/Documents/l'été 2026.pdf");
        assert_eq!(acts[2]["action"]["type"], "terminal");
        assert_eq!(acts[2]["action"]["cwd"], "/h/Documents");
        assert_eq!(
            acts[3]["action"]["command"],
            format!("poste files reveal {quoted}")
        );
        // No `path`: the launcher would add its own "Copy path" a second time.
        assert!(v.get("path").is_none());
    }

    #[test]
    fn folder_outside_home() {
        let env = Env::with_home("/h");
        let v = to_json(
            &env,
            &Hit {
                path: "/srv/data/photos".into(),
                is_dir: true,
            },
        );
        assert_eq!(v["icon"], "folder");
        assert_eq!(v["subtitle"], "/srv/data");
        assert_eq!(v["action"]["command"], "poste files open /srv/data/photos");
        assert_eq!(v["actions"][2]["action"]["cwd"], "/srv/data/photos");
    }

    #[test]
    fn icons_by_type() {
        assert_eq!(icon_for("/a/b.RS", false), "text-x-script");
        assert_eq!(icon_for("/a/photo.JPG", false), "image-x-generic");
        assert_eq!(icon_for("/a/Makefile", false), "text-x-generic");
        assert_eq!(icon_for("/a/b.tar", false), "package-x-generic");
    }
}
