//! Petites équivalences Python utilisées pour rester identique au script d'origine :
//! `str()`, vérité d'une valeur JSON, `json.dumps`, `os.path.*`, `round`.

use serde::Serialize;
use serde_json::ser::{Formatter, Serializer};
use serde_json::Value;
use std::io;

/// `str(valeur)` d'une valeur JSON chargée par `json.load`.
pub fn py_str(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(true) => "True".into(),
        Value::Bool(false) => "False".into(),
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                let f = n.as_f64().unwrap_or(0.0);
                if f.fract() == 0.0 && f.abs() < 1e16 {
                    format!("{f:.1}")
                } else {
                    format!("{f}")
                }
            }
        }
        other => other.to_string(),
    }
}

/// `str(valeur.get(clé, défaut))`.
pub fn py_str_opt(v: Option<&Value>, default: &str) -> String {
    v.map(py_str).unwrap_or_else(|| default.to_string())
}

/// `bool(valeur)` en Python.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

pub fn truthy_opt(v: Option<&Value>) -> bool {
    v.is_some_and(truthy)
}

/// `a == b` en Python pour des valeurs JSON (1 == 1.0, `None` égal à `None`).
pub fn py_eq(a: Option<&Value>, b: Option<&Value>) -> bool {
    fn norm(v: Option<&Value>) -> Option<&Value> {
        v.filter(|x| !x.is_null())
    }
    match (norm(a), norm(b)) {
        (None, None) => true,
        (Some(Value::Number(x)), Some(Value::Number(y))) => x.as_f64() == y.as_f64(),
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// `round(x)` de Python 3 : arrondi au pair le plus proche sur les demi-valeurs.
pub fn py_round(x: f64) -> i64 {
    x.round_ties_even() as i64
}

/// `texte[:n]` en Python (n caractères, pas n octets).
pub fn head_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// `os.path.dirname`.
pub fn dirname(p: &str) -> String {
    let i = p.rfind('/').map(|i| i + 1).unwrap_or(0);
    let head = &p[..i];
    if !head.is_empty() && head.chars().any(|c| c != '/') {
        head.trim_end_matches('/').to_string()
    } else {
        head.to_string()
    }
}

/// `os.path.basename`.
pub fn basename(p: &str) -> String {
    p[p.rfind('/').map(|i| i + 1).unwrap_or(0)..].to_string()
}

/// `os.path.join(a, b)` pour un `b` relatif.
pub fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') || a.is_empty() {
        b.to_string()
    } else if a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

/// Formateur qui reproduit `json.dumps` (séparateurs `, ` et `: `, `ensure_ascii`).
struct Py;

impl Formatter for Py {
    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            w.write_all(b", ")
        }
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if first {
            Ok(())
        } else {
            w.write_all(b", ")
        }
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, w: &mut W) -> io::Result<()> {
        w.write_all(b": ")
    }

    fn write_string_fragment<W: ?Sized + io::Write>(
        &mut self,
        w: &mut W,
        fragment: &str,
    ) -> io::Result<()> {
        let mut buf = [0u16; 2];
        for ch in fragment.chars() {
            if (' '..='~').contains(&ch) {
                w.write_all(&[ch as u8])?;
            } else {
                for unit in ch.encode_utf16(&mut buf) {
                    write!(w, "\\u{unit:04x}")?;
                }
            }
        }
        Ok(())
    }
}

/// `json.dumps(valeur)`.
pub fn dumps<T: Serialize>(v: &T) -> String {
    let mut out = Vec::new();
    let mut ser = Serializer::with_formatter(&mut out, Py);
    v.serialize(&mut ser).expect("sérialisation JSON");
    String::from_utf8(out).expect("JSON en UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn str_python() {
        assert_eq!(py_str(&json!(60)), "60");
        assert_eq!(py_str(&json!(60.0)), "60.0");
        assert_eq!(py_str(&json!(59.94)), "59.94");
        assert_eq!(py_str(&json!(true)), "True");
        assert_eq!(py_str(&Value::Null), "None");
        assert_eq!(py_str(&json!("x")), "x");
    }

    #[test]
    fn verite() {
        assert!(!truthy(&json!(0)));
        assert!(!truthy(&json!("")));
        assert!(!truthy(&json!([])));
        assert!(truthy(&json!({"a": 1})));
        assert!(truthy(&json!("0")));
    }

    #[test]
    fn chemins() {
        assert_eq!(dirname("/a/b/c.mp4"), "/a/b");
        assert_eq!(dirname("c.mp4"), "");
        assert_eq!(dirname("/c"), "/");
        assert_eq!(dirname("/a//b"), "/a");
        assert_eq!(basename("/a/b/c.mp4"), "c.mp4");
        assert_eq!(join("/a/", "b"), "/a/b");
        assert_eq!(join("/a", "b"), "/a/b");
    }

    #[test]
    fn arrondi_au_pair() {
        assert_eq!(py_round(0.5), 0);
        assert_eq!(py_round(1.5), 2);
        assert_eq!(py_round(2.5), 2);
        assert_eq!(py_round(1280.4), 1280);
    }

    #[test]
    fn dumps_comme_python() {
        let v = json!({"a": [1, 2], "b": "é\u{f044b}\"\n\u{7f}"});
        // Ordre des clés : serde_json trie ; on ne teste ici que les séparateurs et l'échappement.
        assert_eq!(
            dumps(&v),
            "{\"a\": [1, 2], \"b\": \"\\u00e9\\udb81\\udc4b\\\"\\n\\u007f\"}"
        );
    }
}
