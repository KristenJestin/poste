//! Screen-share picker for xdg-desktop-portal-hyprland (hidden subcommand
//! `share-picker`, called by `bin/poste-share-picker`).
//!
//! If `poste record` has just requested a window (file `pick.json` less than 20 s old in
//! `$XDG_RUNTIME_DIR/poste-record`), answers directly with that window, showing nothing.
//! Otherwise (screen share of a browser, of OBS…), hands over to
//! `hyprland-preview-share-picker`.
//!
//! xdph protocol: the list of windows arrives in `XDPH_WINDOW_SHARING_LIST`,
//! "handle[HC>]class[HT>]title[HE>]address[HA>]" one after the other; the answer is
//! `[SELECTION]r/window:<handle>` on the standard output.

use crate::env::Env;
use crate::pyval::py_str;
use serde_json::Value;
use std::os::unix::process::CommandExt;
use std::process::Command;

const FALLBACK: &str = "hyprland-preview-share-picker";

/// Python's `int(text, 0)` then `int(text, 16)`.
pub fn as_int(text: &str) -> Option<i128> {
    let t = text.trim();
    let (neg, body) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let lower = body.to_lowercase();
    let base0 = if let Some(h) = lower.strip_prefix("0x") {
        i128::from_str_radix(h, 16).ok()
    } else if let Some(o) = lower.strip_prefix("0o") {
        i128::from_str_radix(o, 8).ok()
    } else if let Some(b) = lower.strip_prefix("0b") {
        i128::from_str_radix(b, 2).ok()
    } else if lower.len() > 1 && lower.starts_with('0') && lower.chars().any(|c| c != '0') {
        None // "0123" is invalid in base 0
    } else if !lower.is_empty() && lower.chars().all(|c| c.is_ascii_digit()) {
        lower.parse::<i128>().ok()
    } else {
        None
    };
    let v = base0.or_else(|| {
        let h = lower.strip_prefix("0x").unwrap_or(&lower);
        i128::from_str_radix(h, 16).ok()
    })?;
    Some(if neg { -v } else { v })
}

/// Looks for the wanted window in the xdph list: the answer `[SELECTION]r/window:<handle>`.
pub fn select(list: &str, want: Option<i128>) -> Option<String> {
    for entry in list.split("[HA>]") {
        if !entry.contains("[HE>]") {
            continue;
        }
        let handle = entry.split("[HC>]").next().unwrap_or("");
        let addr = entry.rsplit("[HE>]").next().unwrap_or("");
        if want.is_some() && as_int(addr) == want {
            return Some(format!("[SELECTION]r/window:{handle}"));
        }
    }
    None
}

fn fallback(args: &[String]) -> ! {
    let err = Command::new(FALLBACK).args(args).exec();
    eprintln!("poste-share-picker: {FALLBACK}: {err}");
    std::process::exit(127);
}

pub fn run(env: &Env, args: &[String]) -> ! {
    let pick_path = env.pick_file();
    let pick: Value = match std::fs::read_to_string(&pick_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
    {
        Some(p) => p,
        None => fallback(args),
    };
    let _ = std::fs::remove_file(&pick_path);
    let at = pick.get("at").and_then(|a| a.as_f64()).unwrap_or(0.0);
    if crate::proc::now_ms() / 1000.0 - at > 20.0 {
        fallback(args);
    }
    let want = match pick.get("address") {
        Some(Value::Null) | None => None,
        Some(a) => as_int(&py_str(a)),
    };
    let list = std::env::var("XDPH_WINDOW_SHARING_LIST").unwrap_or_default();
    // Trace of the last choice (window titles only), for troubleshooting.
    let want_text = pick
        .get("address")
        .map(py_str)
        .unwrap_or_else(|| "None".into());
    let _ = std::fs::write(
        env.run.join("picker.log"),
        format!("want={want_text}\n{list}\n"),
    );
    match select(&list, want) {
        Some(answer) => {
            println!("{answer}");
            std::process::exit(0);
        }
        None => fallback(args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integers_like_python() {
        assert_eq!(as_int("0x1f"), Some(31));
        assert_eq!(as_int(" 42 "), Some(42));
        assert_eq!(as_int("0123"), Some(0x123)); // fallback to hexadecimal
        assert_eq!(as_int("abc"), Some(0xabc));
        assert_eq!(as_int("zz"), None);
        assert_eq!(as_int(""), None);
        assert_eq!(as_int("0"), Some(0));
    }

    #[test]
    fn window_choice() {
        let list = "12[HC>]kitty[HT>]shell[HE>]55aa[HA>]34[HC>]Unity[HT>]Project[HE>]0x5581[HA>]";
        assert_eq!(
            select(list, as_int("0x5581")).as_deref(),
            Some("[SELECTION]r/window:34")
        );
        assert_eq!(select(list, as_int("0x9999")), None);
        assert_eq!(select(list, None), None);
    }
}
