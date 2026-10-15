//! Per-machine JSON overrides: a JSON file of `config/` that has `[[override]]` entries in
//! `~/.config/poste/config.toml` is not linked; `setup` writes an ordinary file equal to the
//! content of the repository plus the overridden values (dotted keys, `idle.lock`).
//!
//! `--take-home` on such a file: the file of `~` is taken back into the repository AFTER having
//! put back, for each overridden key, the value of the repository (or removed the key if the repository
//! did not have it). Refused, without writing anything, if an overridden value was changed by hand in
//! `~` (or if the key has disappeared): we cannot guess whether the intention is to change the
//! override or the repository.

use crate::ojson::{self, J};
use anyhow::{bail, Context, Result};
use poste_core::config::Override;

/// An override ready to apply: (dotted key, value).
pub type Sets = Vec<(String, J)>;

fn toml_to_j(v: &toml::Value) -> Result<J> {
    Ok(match v {
        toml::Value::String(s) => J::Str(s.clone()),
        toml::Value::Integer(i) => J::Num(i.to_string()),
        toml::Value::Float(f) => J::Num(format!("{f}")),
        toml::Value::Boolean(b) => J::Bool(*b),
        toml::Value::Datetime(d) => J::Str(d.to_string()),
        toml::Value::Array(a) => J::Arr(a.iter().map(toml_to_j).collect::<Result<_>>()?),
        toml::Value::Table(t) => J::Obj(
            t.iter()
                .map(|(k, v)| Ok((k.clone(), toml_to_j(v)?)))
                .collect::<Result<_>>()?,
        ),
    })
}

fn flatten(prefix: &str, t: &toml::Table, out: &mut Sets) -> Result<()> {
    for (k, v) in t {
        let key = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            toml::Value::Table(sub) => flatten(&key, sub, out)?,
            other => out.push((key, toml_to_j(other)?)),
        }
    }
    Ok(())
}

/// The overrides of an `[[override]]`, as dotted keys.
pub fn sets_of(o: &Override) -> Result<Sets> {
    let mut out = Vec::new();
    flatten("", &o.set, &mut out)?;
    for (k, _) in &out {
        if k.is_empty() || k.split('.').any(str::is_empty) {
            bail!("override {}: invalid key '{k}'", o.path);
        }
    }
    Ok(out)
}

fn parts(k: &str) -> Vec<&str> {
    k.split('.').collect()
}

/// Content of the repository plus the overrides, JSON rewritten cleanly (indentation 2).
pub fn generate(repo_text: &str, sets: &Sets) -> Result<String> {
    let mut j = ojson::parse(repo_text).context("invalid JSON in the repository")?;
    for (k, v) in sets {
        j.set(&parts(k), v.clone())
            .with_context(|| format!("override '{k}'"))?;
    }
    Ok(ojson::pretty(&j))
}

/// The repository text to write for a `--take-home`: `home_text` with the overridden keys
/// put back as in the repository. Error if ambiguous (see the module header).
pub fn take_home_text(home_text: &str, repo_text: &str, sets: &Sets) -> Result<String> {
    let mut home = ojson::parse(home_text).context("the file of ~ is not valid JSON")?;
    let repo = ojson::parse(repo_text).context("invalid JSON in the repository")?;
    let mut changed = Vec::new();
    for (k, v) in sets {
        match home.get(&parts(k)) {
            Some(cur) if cur.same(v) => {}
            Some(_) => changed.push(format!("{k} has another value than the override")),
            None => changed.push(format!("{k} has disappeared from the file")),
        }
    }
    if !changed.is_empty() {
        bail!(
            "ambiguous take-back: {}. Fix the override (poste sys override set/unset) or use --take-repo",
            changed.join("; ")
        );
    }
    for (k, _) in sets {
        match repo.get(&parts(k)) {
            Some(orig) => home.set(&parts(k), orig.clone())?,
            None => {
                home.remove(&parts(k));
            }
        }
    }
    Ok(ojson::pretty(&home))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ov(toml_text: &str) -> Override {
        poste_core::Config::parse(toml_text)
            .unwrap()
            .overrides
            .remove(0)
    }

    #[test]
    fn cles_pointees_et_tables_aplaties() {
        let a = ov(
            "[[override]]\npath = \"x.json\"\nset = { \"idle.lock\" = 3600, \"a.b\" = \"t\" }\n",
        );
        let s = sets_of(&a).unwrap();
        assert!(s.contains(&("idle.lock".into(), J::Num("3600".into()))));
        let b = ov("[[override]]\npath = \"x.json\"\n[override.set.idle]\nlock = 1\n");
        assert_eq!(
            sets_of(&b).unwrap(),
            [("idle.lock".into(), J::Num("1".into()))]
        );
        let c = ov("[[override]]\npath = \"x.json\"\nset = { \"a..b\" = 1 }\n");
        assert!(sets_of(&c).is_err());
    }

    const REPO: &str = "{\n  \"bar\": {\"x\": 1},\n  \"idle\": {\n    \"lock\": 5400,\n    \"screensaver\": 2700\n  }\n}\n";

    #[test]
    fn generation() {
        let s: Sets = vec![("idle.lock".into(), J::Num("3600".into()))];
        let g = generate(REPO, &s).unwrap();
        assert!(g.contains("\"lock\": 3600") && g.contains("\"screensaver\": 2700"));
        assert!(g.find("bar").unwrap() < g.find("idle").unwrap());
        // Idempotent.
        assert_eq!(generate(&g, &s).unwrap(), g);
    }

    #[test]
    fn take_back_restores_repository_values() {
        let s: Sets = vec![
            ("idle.lock".into(), J::Num("3600".into())),
            ("fresh.key".into(), J::Bool(true)),
        ];
        let home = generate(REPO, &s).unwrap().replace("\"x\": 1", "\"x\": 2");
        let back = take_home_text(&home, REPO, &s).unwrap();
        assert!(back.contains("\"lock\": 5400") && back.contains("\"x\": 2"));
        assert!(!back.contains("fresh"));
    }

    #[test]
    fn ambiguous_take_back_refused() {
        let s: Sets = vec![("idle.lock".into(), J::Num("3600".into()))];
        let home = generate(REPO, &s).unwrap().replace("3600", "7200");
        let e = take_home_text(&home, REPO, &s).unwrap_err().to_string();
        assert!(e.contains("idle.lock") && e.contains("ambiguous"));
        assert!(take_home_text("not json", REPO, &s).is_err());
    }
}
