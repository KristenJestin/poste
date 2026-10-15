//! Windows and screens from `hyprctl … -j`, and highlight rectangles.

use crate::proc::{capture, prog, HYPRCTL};
use crate::pyval::{py_eq, py_round, py_str_opt, truthy};
use regex::RegexBuilder;
use serde::Serialize;
use serde_json::{Map, Value};
use std::time::Duration;

pub type Obj = Map<String, Value>;

fn hyprctl_list(what: &str) -> Vec<Obj> {
    let Some(out) = capture(&prog(HYPRCTL), &[what, "-j"], Duration::from_secs(3)) else {
        return Vec::new();
    };
    match serde_json::from_str::<Value>(&out) {
        Ok(Value::Array(a)) => a
            .into_iter()
            .filter_map(|v| {
                if let Value::Object(o) = v {
                    Some(o)
                } else {
                    None
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

pub fn clients() -> Vec<Obj> {
    hyprctl_list("clients")
}

pub fn monitors() -> Vec<Obj> {
    hyprctl_list("monitors")
}

/// `str(c.get(key, ""))`.
pub fn text(c: &Obj, key: &str) -> String {
    py_str_opt(c.get(key), "")
}

/// `c.get("mapped", True)`.
pub fn mapped(c: &Obj) -> bool {
    c.get("mapped").is_none_or(truthy)
}

/// Number of a field (0 by default), for the size.
fn pair(c: &Obj, key: &str) -> (Value, Value) {
    match c.get(key) {
        Some(Value::Array(a)) if a.len() >= 2 => (a[0].clone(), a[1].clone()),
        _ => (Value::from(0), Value::from(0)),
    }
}

/// `find_window(match)`: the largest open window that matches `class` and `title`
/// (regular expressions, case-insensitive). An invalid expression matches nothing.
pub fn find_window(m: &Value, all: &[Obj]) -> Option<Obj> {
    let get = |k: &str| m.get(k).filter(|v| truthy(v)).map(crate::pyval::py_str);
    let (cls, title) = (get("class"), get("title"));
    let build = |p: &Option<String>| -> Result<Option<regex::Regex>, ()> {
        match p {
            None => Ok(None),
            Some(p) => RegexBuilder::new(p)
                .case_insensitive(true)
                .build()
                .map(Some)
                .map_err(|_| ()),
        }
    };
    let (Ok(cls_re), Ok(title_re)) = (build(&cls), build(&title)) else {
        return None;
    };
    let mut found: Vec<&Obj> = all
        .iter()
        .filter(|c| mapped(c))
        .filter(|c| {
            cls_re
                .as_ref()
                .is_none_or(|r| r.is_match(&text(c, "class")))
        })
        .filter(|c| {
            title_re
                .as_ref()
                .is_none_or(|r| r.is_match(&text(c, "title")))
        })
        .collect();
    // The largest: the main window rather than a dialog box.
    let area = |c: &Obj| {
        let (w, h) = pair(c, "size");
        -(w.as_f64().unwrap_or(0.0) * h.as_f64().unwrap_or(0.0))
    };
    found.sort_by(|a, b| area(a).total_cmp(&area(b)));
    found.first().map(|c| (*c).clone())
}

/// Highlight rectangle: `x, y, w, h` (and `window: true` for a window).
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Rect {
    pub x: Value,
    pub y: Value,
    pub w: Value,
    pub h: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<bool>,
}

/// Logical rectangle of a screen (rotation and scale included).
pub fn monitor_rect(m: &Obj) -> Rect {
    let scale = m
        .get("scale")
        .filter(|v| truthy(v))
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);
    let dim = |k: &str| m.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0) / scale;
    let (mut w, mut h) = (dim("width"), dim("height"));
    let transform = m
        .get("transform")
        .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
        .unwrap_or(0);
    if transform.rem_euclid(2) == 1 {
        std::mem::swap(&mut w, &mut h);
    }
    Rect {
        x: m.get("x").cloned().unwrap_or_else(|| Value::from(0)),
        y: m.get("y").cloned().unwrap_or_else(|| Value::from(0)),
        w: Value::from(py_round(w)),
        h: Value::from(py_round(h)),
        window: None,
    }
}

/// Rectangle of a window, only if its workspace is shown somewhere.
pub fn window_rect(c: &Obj, mons: &[Obj]) -> Option<Rect> {
    let id_of = |v: Option<&Value>| -> Option<Value> {
        match v {
            Some(Value::Object(o)) if !o.is_empty() => o.get("id").cloned(),
            _ => None,
        }
    };
    let ws = id_of(c.get("workspace"));
    let shown = mons.iter().any(|m| {
        let active = id_of(m.get("activeWorkspace"));
        let special = id_of(m.get("specialWorkspace"));
        py_eq(ws.as_ref(), active.as_ref()) || py_eq(ws.as_ref(), special.as_ref())
    });
    if !shown {
        return None;
    }
    let (x, y) = pair(c, "at");
    let (w, h) = pair(c, "size");
    Some(Rect {
        x,
        y,
        w,
        h,
        window: Some(true),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Obj {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn rectangle_screen_scale_and_rotation() {
        let m = obj(
            json!({"x": 1920, "y": 0, "width": 3840, "height": 2160, "scale": 1.5, "transform": 0}),
        );
        let r = monitor_rect(&m);
        assert_eq!((r.w, r.h), (json!(2560), json!(1440)));
        let m = obj(
            json!({"x": 0, "y": 0, "width": 3840, "height": 2160, "scale": 1.5, "transform": 1}),
        );
        let r = monitor_rect(&m);
        assert_eq!((r.w, r.h), (json!(1440), json!(2560)));
        // Negative or flipped transform (>= 4): only the parity counts, as in Python.
        let m = obj(json!({"width": 1000, "height": 500, "scale": 0, "transform": 3}));
        let r = monitor_rect(&m);
        assert_eq!((r.w, r.h), (json!(500), json!(1000)));
    }

    #[test]
    fn fenetre_visible_seulement_si_son_bureau_est_affiche() {
        let mons = vec![obj(
            json!({"activeWorkspace": {"id": 2}, "specialWorkspace": {"id": 0}}),
        )];
        let c = obj(json!({"workspace": {"id": 2}, "at": [10, 20], "size": [300, 400]}));
        let r = window_rect(&c, &mons).unwrap();
        assert_eq!(r.window, Some(true));
        assert_eq!((r.x, r.w), (json!(10), json!(300)));
        let c = obj(json!({"workspace": {"id": 3}, "at": [0, 0], "size": [1, 1]}));
        assert!(window_rect(&c, &mons).is_none());
    }

    #[test]
    fn plus_grande_fenetre() {
        let all = vec![
            obj(json!({"class": "Unity", "mapped": true, "size": [100, 100], "address": "a"})),
            obj(json!({"class": "Unity", "mapped": true, "size": [800, 600], "address": "b"})),
            obj(json!({"class": "Unity", "mapped": false, "size": [9000, 9000], "address": "c"})),
            obj(json!({"class": "Other", "mapped": true, "size": [9999, 9999], "address": "d"})),
        ];
        let w = find_window(&json!({"class": "^unity$"}), &all).unwrap();
        assert_eq!(w["address"], "b");
        assert!(find_window(&json!({"class": "("}), &all).is_none());
    }
}
