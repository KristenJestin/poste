//! JSON minimal qui garde l'ORDRE des clés et le texte exact des nombres : sert à générer un
//! fichier de config (dépôt + surcharges) sans le réordonner ni le déformer. Pas de dépendance
//! (`serde_json` trierait les clés, et l'activer ici changerait les autres modules du dépôt).

use anyhow::{bail, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    /// Texte du nombre tel qu'écrit.
    Num(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

struct P<'a> {
    s: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn eat(&mut self, c: u8) -> Result<()> {
        self.ws();
        if self.s.get(self.i) == Some(&c) {
            self.i += 1;
            Ok(())
        } else {
            bail!("« {} » attendu à l'octet {}", c as char, self.i)
        }
    }
    fn value(&mut self) -> Result<J> {
        self.ws();
        match self.s.get(self.i) {
            None => bail!("fin inattendue"),
            Some(b'{') => {
                self.i += 1;
                let mut v = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(J::Obj(v));
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.eat(b':')?;
                    let val = self.value()?;
                    v.push((k, val));
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(J::Obj(v));
                        }
                        _ => bail!("« , » ou « }} » attendu à l'octet {}", self.i),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                let mut v = Vec::new();
                self.ws();
                if self.s.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(J::Arr(v));
                }
                loop {
                    v.push(self.value()?);
                    self.ws();
                    match self.s.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(J::Arr(v));
                        }
                        _ => bail!("« , » ou « ] » attendu à l'octet {}", self.i),
                    }
                }
            }
            Some(b'"') => Ok(J::Str(self.string()?)),
            Some(b't') if self.s[self.i..].starts_with(b"true") => {
                self.i += 4;
                Ok(J::Bool(true))
            }
            Some(b'f') if self.s[self.i..].starts_with(b"false") => {
                self.i += 5;
                Ok(J::Bool(false))
            }
            Some(b'n') if self.s[self.i..].starts_with(b"null") => {
                self.i += 4;
                Ok(J::Null)
            }
            Some(c) if c.is_ascii_digit() || *c == b'-' => {
                let st = self.i;
                while self.i < self.s.len()
                    && matches!(
                        self.s[self.i],
                        b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E'
                    )
                {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.s[st..self.i])?;
                if t.parse::<f64>().is_err() {
                    bail!("nombre invalide « {t} »");
                }
                Ok(J::Num(t.to_string()))
            }
            Some(c) => bail!(
                "caractère inattendu « {} » à l'octet {}",
                *c as char,
                self.i
            ),
        }
    }
    fn string(&mut self) -> Result<String> {
        if self.s.get(self.i) != Some(&b'"') {
            bail!("chaîne attendue à l'octet {}", self.i);
        }
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(&c) = self.s.get(self.i) else {
                bail!("chaîne non terminée")
            };
            self.i += 1;
            match c {
                b'"' => return Ok(String::from_utf8(out)?),
                b'\\' => {
                    let Some(&e) = self.s.get(self.i) else {
                        bail!("échappement incomplet")
                    };
                    self.i += 1;
                    match e {
                        b'"' | b'\\' | b'/' => out.push(e),
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                if self.s[self.i..].starts_with(b"\\u") {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    cp =
                                        0x10000 + ((cp - 0xD800) << 10) + (lo.wrapping_sub(0xDC00));
                                } else {
                                    bail!("paire de substitution incomplète");
                                }
                            }
                            let ch = char::from_u32(cp)
                                .ok_or_else(|| anyhow::anyhow!("\\u invalide"))?;
                            let mut b = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                        }
                        _ => bail!("échappement inconnu \\{}", e as char),
                    }
                }
                _ => out.push(c),
            }
        }
    }
    fn hex4(&mut self) -> Result<u32> {
        let h = self
            .s
            .get(self.i..self.i + 4)
            .ok_or_else(|| anyhow::anyhow!("\\u incomplet"))?;
        self.i += 4;
        Ok(u32::from_str_radix(std::str::from_utf8(h)?, 16)?)
    }
}

pub fn parse(text: &str) -> Result<J> {
    let mut p = P {
        s: text.as_bytes(),
        i: 0,
    };
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        bail!("texte en trop à l'octet {}", p.i);
    }
    Ok(v)
}

fn esc(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write(v: &J, depth: usize, out: &mut String) {
    let pad = |n: usize| "  ".repeat(n);
    match v {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Num(n) => out.push_str(n),
        J::Str(s) => esc(s, out),
        J::Arr(a) if a.is_empty() => out.push_str("[]"),
        J::Obj(o) if o.is_empty() => out.push_str("{}"),
        J::Arr(a) => {
            out.push_str("[\n");
            for (i, x) in a.iter().enumerate() {
                out.push_str(&pad(depth + 1));
                write(x, depth + 1, out);
                out.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(depth));
            out.push(']');
        }
        J::Obj(o) => {
            out.push_str("{\n");
            for (i, (k, x)) in o.iter().enumerate() {
                out.push_str(&pad(depth + 1));
                esc(k, out);
                out.push_str(": ");
                write(x, depth + 1, out);
                out.push_str(if i + 1 < o.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(depth));
            out.push('}');
        }
    }
}

/// Indentation de 2 espaces, retour à la ligne final.
pub fn pretty(v: &J) -> String {
    let mut s = String::new();
    write(v, 0, &mut s);
    s.push('\n');
    s
}

impl J {
    /// Égalité sémantique (les nombres se comparent par valeur, l'ordre des clés ne compte pas).
    pub fn same(&self, o: &J) -> bool {
        match (self, o) {
            (J::Num(a), J::Num(b)) => a.parse::<f64>().ok() == b.parse::<f64>().ok(),
            (J::Arr(a), J::Arr(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.same(y)),
            (J::Obj(a), J::Obj(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(k, v)| b.iter().any(|(k2, v2)| k == k2 && v.same(v2)))
            }
            _ => self == o,
        }
    }

    pub fn get(&self, path: &[&str]) -> Option<&J> {
        let Some((k, rest)) = path.split_first() else {
            return Some(self);
        };
        match self {
            J::Obj(o) => o.iter().find(|(n, _)| n == k)?.1.get(rest),
            _ => None,
        }
    }

    /// Pose `val` au chemin (objets intermédiaires créés) ; erreur si un maillon n'est pas un objet.
    pub fn set(&mut self, path: &[&str], val: J) -> Result<()> {
        let Some((k, rest)) = path.split_first() else {
            *self = val;
            return Ok(());
        };
        let J::Obj(o) = self else {
            bail!("« {k} » : le parent n'est pas un objet");
        };
        if let Some((_, v)) = o.iter_mut().find(|(n, _)| n == k) {
            return v.set(rest, val);
        }
        let mut child = J::Obj(vec![]);
        child.set(rest, val)?;
        o.push((k.to_string(), child));
        Ok(())
    }

    /// Retire la clé et les objets parents devenus vides ; `true` si elle existait.
    pub fn remove(&mut self, path: &[&str]) -> bool {
        let Some((k, rest)) = path.split_first() else {
            return false;
        };
        let J::Obj(o) = self else { return false };
        let Some(pos) = o.iter().position(|(n, _)| n == k) else {
            return false;
        };
        if rest.is_empty() {
            o.remove(pos);
            return true;
        }
        let r = o[pos].1.remove(rest);
        if r && matches!(&o[pos].1, J::Obj(c) if c.is_empty()) {
            o.remove(pos);
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordre_nombres_et_unicode_conserves() {
        let t = "{\n  \"z\": 1.50,\n  \"a\": [1, {\"k\": \"é ✓ \\n\"}],\n  \"e\": {},\n  \"n\": null\n}";
        let v = parse(t).unwrap();
        let out = pretty(&v);
        assert!(out.find("\"z\"").unwrap() < out.find("\"a\"").unwrap());
        assert!(out.contains("1.50") && out.contains("é ✓ \\n") && out.contains("\"e\": {}"));
        assert!(parse(&out).unwrap().same(&v));
        assert_eq!(pretty(&parse(&out).unwrap()), out);
    }

    #[test]
    fn erreurs() {
        assert!(parse("{").is_err());
        assert!(parse("{\"a\":1} x").is_err());
        assert!(parse("[1,]").is_err());
    }

    #[test]
    fn get_set_remove() {
        let mut v = parse("{\"idle\": {\"lock\": 5400, \"screensaver\": 2700}}").unwrap();
        v.set(&["idle", "lock"], J::Num("3600".into())).unwrap();
        assert_eq!(v.get(&["idle", "lock"]), Some(&J::Num("3600".into())));
        v.set(&["a", "b", "c"], J::Bool(true)).unwrap();
        assert!(v.set(&["idle", "lock", "x"], J::Null).is_err());
        assert!(v.remove(&["a", "b", "c"]));
        assert!(v.get(&["a"]).is_none(), "parents vides retirés");
        assert!(!v.remove(&["nope"]));
        assert!(parse("1.0").unwrap().same(&parse("1").unwrap()));
    }

    #[test]
    fn surrogates() {
        assert_eq!(parse("\"\\ud83d\\ude00\"").unwrap(), J::Str("😀".into()));
    }
}
