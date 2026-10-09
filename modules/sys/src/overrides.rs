//! Surcharges JSON par machine : un fichier de `config/` (JSON) qui a des `[[override]]` dans
//! `~/.config/poste/config.toml` n'est pas lié ; `setup` écrit un fichier ordinaire égal au
//! contenu du dépôt plus les valeurs surchargées (clés pointées, `idle.lock`).
//!
//! `--take-home` sur un tel fichier : le fichier de `~` est repris dans le dépôt APRÈS avoir
//! remis, pour chaque clé surchargée, la valeur du dépôt (ou retiré la clé si le dépôt ne
//! l'avait pas). Refusé, sans rien écrire, si une valeur surchargée a été changée à la main dans
//! `~` (ou si la clé a disparu) : on ne peut pas deviner si l'intention est de changer la
//! surcharge ou le dépôt.

use crate::ojson::{self, J};
use anyhow::{bail, Context, Result};
use poste_core::config::Override;

/// Une surcharge prête à appliquer : (clé pointée, valeur).
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

/// Les surcharges d'un `[[override]]`, en clés pointées.
pub fn sets_of(o: &Override) -> Result<Sets> {
    let mut out = Vec::new();
    flatten("", &o.set, &mut out)?;
    for (k, _) in &out {
        if k.is_empty() || k.split('.').any(str::is_empty) {
            bail!("override {} : clé invalide « {k} »", o.path);
        }
    }
    Ok(out)
}

fn parts(k: &str) -> Vec<&str> {
    k.split('.').collect()
}

/// Contenu du dépôt plus les surcharges, JSON réécrit proprement (indentation 2).
pub fn generate(repo_text: &str, sets: &Sets) -> Result<String> {
    let mut j = ojson::parse(repo_text).context("JSON du dépôt invalide")?;
    for (k, v) in sets {
        j.set(&parts(k), v.clone())
            .with_context(|| format!("surcharge « {k} »"))?;
    }
    Ok(ojson::pretty(&j))
}

/// Le texte du dépôt à écrire pour un `--take-home` : `home_text` avec les clés surchargées
/// remises comme dans le dépôt. Erreur si ambigu (voir l'en-tête du module).
pub fn take_home_text(home_text: &str, repo_text: &str, sets: &Sets) -> Result<String> {
    let mut home = ojson::parse(home_text).context("le fichier de ~ n'est pas du JSON valide")?;
    let repo = ojson::parse(repo_text).context("JSON du dépôt invalide")?;
    let mut changed = Vec::new();
    for (k, v) in sets {
        match home.get(&parts(k)) {
            Some(cur) if cur.same(v) => {}
            Some(_) => changed.push(format!("{k} a une autre valeur que la surcharge")),
            None => changed.push(format!("{k} a disparu du fichier")),
        }
    }
    if !changed.is_empty() {
        bail!(
            "reprise ambiguë : {}. Corrigez la surcharge (poste sys override set/unset) ou utilisez --take-repo",
            changed.join(" ; ")
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
    fn reprise_remet_les_valeurs_du_depot() {
        let s: Sets = vec![
            ("idle.lock".into(), J::Num("3600".into())),
            ("neuf.cle".into(), J::Bool(true)),
        ];
        let home = generate(REPO, &s).unwrap().replace("\"x\": 1", "\"x\": 2");
        let back = take_home_text(&home, REPO, &s).unwrap();
        assert!(back.contains("\"lock\": 5400") && back.contains("\"x\": 2"));
        assert!(!back.contains("neuf"));
    }

    #[test]
    fn reprise_ambigue_refusee() {
        let s: Sets = vec![("idle.lock".into(), J::Num("3600".into()))];
        let home = generate(REPO, &s).unwrap().replace("3600", "7200");
        let e = take_home_text(&home, REPO, &s).unwrap_err().to_string();
        assert!(e.contains("idle.lock") && e.contains("ambiguë"));
        assert!(take_home_text("pas du json", REPO, &s).is_err());
    }
}
