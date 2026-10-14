//! Réglages propres à cette machine, dans `~/.config/poste/config.toml` (jamais dans le dépôt) :
//! `local` (fichiers de `config/` gardés à la machine) et `disabled` (modules non installés).
//! `poste sys local …` et `poste sys module …` éditent ces deux listes sans toucher au reste
//! du fichier (commentaires et autres clés conservés).

use crate::config;
use crate::ojson::{self, J};
use crate::plan;
use crate::provision;
use anyhow::{bail, Context, Result};
use poste_core::{Config, Env};
use std::path::Path;
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

/// Ajoute (`add = true`) ou retire `value` de la liste `key` du fichier. Renvoie `true` si le
/// fichier a changé. Le fichier est créé au besoin ; écriture atomique.
pub fn edit_list(file: &Path, key: &str, value: &str, add: bool) -> Result<bool> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("lecture de {}", file.display())),
    };
    let mut doc: DocumentMut = text
        .parse()
        .with_context(|| format!("{} invalide", file.display()))?;
    let present = doc
        .get(key)
        .and_then(|i| i.as_array())
        .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(value)));
    if present == add {
        return Ok(false);
    }
    if add {
        if doc.get(key).is_none() {
            doc[key] = Item::Value(Value::Array(Array::new()));
        }
        let Some(arr) = doc[key].as_array_mut() else {
            bail!("{} : « {key} » n'est pas une liste", file.display());
        };
        arr.push(value);
    } else if let Some(arr) = doc[key].as_array_mut() {
        arr.retain(|v| v.as_str() != Some(value));
    }
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("toml.tmp");
    std::fs::write(&tmp, doc.to_string())?;
    std::fs::rename(&tmp, file)?;
    Ok(true)
}

/// Fixe la clé de premier niveau `key = "value"` (chaîne) sans toucher au reste du fichier.
/// Renvoie `true` si le fichier a changé. Le fichier est créé au besoin ; écriture atomique.
pub fn set_key(file: &Path, key: &str, value: &str) -> Result<bool> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("lecture de {}", file.display())),
    };
    let mut doc: DocumentMut = text
        .parse()
        .with_context(|| format!("{} invalide", file.display()))?;
    if doc.get(key).and_then(|i| i.as_str()) == Some(value) {
        return Ok(false);
    }
    doc[key] = toml_edit::value(value);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("toml.tmp");
    std::fs::write(&tmp, doc.to_string())?;
    std::fs::rename(&tmp, file)?;
    Ok(true)
}

// ---------------------------------------------------------------- local

/// `poste sys local` : liste les fichiers locaux.
pub fn local_list(env: &Env, repo: &Path) -> Result<()> {
    let Some(layout) = config::load(repo, env)? else {
        bail!("pas de dossier config/ dans le dépôt");
    };
    if layout.local_paths.is_empty() {
        println!("Aucun fichier local : tout config/ est lié (poste sys local add <chemin> pour en garder un à cette machine).");
        return Ok(());
    }
    config::print_local(&layout, env)
}

/// `poste sys local add <chemin>`.
pub fn local_add(env: &Env, repo: &Path, arg: &str) -> Result<()> {
    let rel = config::rel_of(env, arg)?;
    let Some(all) = config::load_with(repo, env, &[], &[])? else {
        bail!("pas de dossier config/ dans le dépôt");
    };
    if let Some(d) = all
        .link_dirs
        .iter()
        .find(|d| rel.starts_with(&format!("{d}/")))
    {
        bail!("{rel} est dans le dossier lié d'un bloc « {d} » : déclarez ce dossier comme local");
    }
    if !all
        .entries
        .iter()
        .any(|e| config::is_local(&e.rel, std::slice::from_ref(&rel)))
    {
        bail!("{rel} n'est pas une entrée de config/ du dépôt (rien à garder en local)");
    }
    let changed = edit_list(&env.config_file(), "local", &rel, true)?;
    let tag = if changed { "ajouté à" } else { "déjà dans" };
    println!("{rel} : {tag} local ({}).", env.display(&env.config_file()));
    // État actuel de la cible, pour dire quoi faire du lien existant.
    for e in all
        .entries
        .iter()
        .filter(|e| config::is_local(&e.rel, std::slice::from_ref(&rel)))
    {
        if config::state_of(e, env)? == config::State::Linked {
            println!(
                "  {} est encore un lien vers le dépôt : poste n'y touchera plus, mais l'éditer modifierait le dépôt. Pour en faire un fichier ordinaire (copie), lancez `poste sys forget {}`.",
                env.display(&e.dest),
                e.rel
            );
        }
    }
    println!("  setup ne le lie plus, ne le signale plus en conflit et ne le touche jamais.");
    Ok(())
}

/// `poste sys local remove <chemin>`.
pub fn local_remove(env: &Env, arg: &str) -> Result<()> {
    let rel = config::rel_of(env, arg)?;
    if !edit_list(&env.config_file(), "local", &rel, false)? {
        bail!(
            "{rel} n'est pas dans local ({})",
            env.display(&env.config_file())
        );
    }
    println!("{rel} : retiré de local.");
    println!("  Le prochain setup le liera au dépôt ; si le fichier de ~ est ordinaire et diffère, ce sera un conflit à trancher (poste sys diff {rel}).");
    Ok(())
}

// ---------------------------------------------------------------- surcharges JSON

fn j_to_toml(v: &J) -> Result<Value> {
    Ok(match v {
        J::Bool(b) => Value::from(*b),
        J::Str(s) => Value::from(s.as_str()),
        J::Num(n) => match n.parse::<i64>() {
            Ok(i) => Value::from(i),
            Err(_) => Value::from(n.parse::<f64>()?),
        },
        J::Arr(a) => {
            let mut out = Array::new();
            for x in a {
                out.push(j_to_toml(x)?);
            }
            Value::Array(out)
        }
        J::Null => bail!("null ne peut pas être une surcharge (TOML n'a pas de null)"),
        J::Obj(_) => {
            bail!("donnez les clés une par une (idle.lock, idle.screensaver…) plutôt qu'un objet")
        }
    })
}

fn find_override<'a>(doc: &'a mut DocumentMut, rel: &str) -> Option<&'a mut Table> {
    let aot = doc.get_mut("override")?.as_array_of_tables_mut()?;
    aot.iter_mut().find(|t| {
        t.get("path")
            .and_then(|p| p.as_str())
            .and_then(|p| config::normalize_rel(p).ok())
            .as_deref()
            == Some(rel)
    })
}

fn write_doc(file: &Path, doc: &DocumentMut) -> Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension("toml.tmp");
    std::fs::write(&tmp, doc.to_string())?;
    std::fs::rename(&tmp, file)?;
    Ok(())
}

fn read_doc(file: &Path) -> Result<DocumentMut> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("lecture de {}", file.display())),
    };
    text.parse()
        .with_context(|| format!("{} invalide", file.display()))
}

fn check_key(key: &str) -> Result<()> {
    if key.is_empty() || key.split('.').any(str::is_empty) {
        bail!("clé invalide : « {key} » (attendu : a.b.c)");
    }
    Ok(())
}

/// `poste sys override set <chemin> <clé> <valeur>` : la valeur est du JSON (3600, true, "texte",
/// [1,2]) ; ce qui n'en est pas devient une chaîne.
pub fn override_set(env: &Env, repo: &Path, arg: &str, key: &str, value: &str) -> Result<()> {
    check_key(key)?;
    let rel = config::rel_of(env, arg)?;
    let Some(all) = config::load_with(repo, env, &[], &[])? else {
        bail!("pas de dossier config/ dans le dépôt");
    };
    let Some(e) = all.entries.iter().find(|e| e.rel == rel) else {
        bail!("{rel} n'est pas un fichier de config/ du dépôt");
    };
    let repo_text = std::fs::read_to_string(&e.src)?;
    ojson::parse(&repo_text)
        .with_context(|| format!("{rel} n'est pas du JSON valide dans le dépôt"))?;
    let j = ojson::parse(value).unwrap_or_else(|_| J::Str(value.to_string()));
    let tv = j_to_toml(&j)?;
    // Le résultat doit rester générable (clé qui traverse une valeur non objet : refus).
    let probe: crate::overrides::Sets = vec![(key.to_string(), j.clone())];
    crate::overrides::generate(&repo_text, &probe)?;

    let file = env.config_file();
    let mut doc = read_doc(&file)?;
    if doc.get("override").is_none() {
        doc["override"] = Item::ArrayOfTables(ArrayOfTables::new());
    }
    if find_override(&mut doc, &rel).is_none() {
        let Some(aot) = doc["override"].as_array_of_tables_mut() else {
            bail!(
                "{} : « override » doit être une liste de [[override]]",
                file.display()
            );
        };
        let mut t = Table::new();
        t["path"] = Item::Value(Value::from(rel.as_str()));
        t["set"] = Item::Value(Value::InlineTable(InlineTable::new()));
        aot.push(t);
    }
    let t = find_override(&mut doc, &rel).expect("table ajoutée");
    if t.get("set").is_none() {
        t["set"] = Item::Value(Value::InlineTable(InlineTable::new()));
    }
    match t.get_mut("set") {
        Some(Item::Value(Value::InlineTable(it))) => {
            it.insert(key, tv);
        }
        Some(Item::Table(tb)) => {
            tb.insert(key, Item::Value(tv));
        }
        _ => bail!("{} : « set » doit être une table", file.display()),
    }
    write_doc(&file, &doc)?;
    println!(
        "{rel} : {key} = {} (surcharge de cette machine, {}).",
        ojson::pretty(&j).trim(),
        env.display(&file)
    );
    println!("  Le fichier sera généré (dépôt + surcharges) au prochain `poste sys setup` ; il n'est plus un lien.");
    Ok(())
}

/// `poste sys override unset <chemin> <clé>`.
pub fn override_unset(env: &Env, arg: &str, key: &str) -> Result<()> {
    let rel = config::rel_of(env, arg)?;
    let file = env.config_file();
    let mut doc = read_doc(&file)?;
    let removed = match find_override(&mut doc, &rel) {
        Some(t) => match t.get_mut("set") {
            Some(Item::Value(Value::InlineTable(it))) => it.remove(key).is_some(),
            Some(Item::Table(tb)) => tb.remove(key).is_some(),
            _ => false,
        },
        None => false,
    };
    if !removed {
        bail!(
            "{rel} : pas de surcharge « {key} » ({})",
            env.display(&file)
        );
    }
    // Plus aucune clé : la table disparaît et le fichier redevient un lien au prochain setup.
    let empty = find_override(&mut doc, &rel).is_some_and(|t| match t.get("set") {
        Some(Item::Value(Value::InlineTable(it))) => it.is_empty(),
        Some(Item::Table(tb)) => tb.is_empty(),
        _ => true,
    });
    if empty {
        if let Some(aot) = doc["override"].as_array_of_tables_mut() {
            let idx = aot.iter().position(|t| {
                t.get("path")
                    .and_then(|p| p.as_str())
                    .and_then(|p| config::normalize_rel(p).ok())
                    .as_deref()
                    == Some(rel.as_str())
            });
            if let Some(i) = idx {
                aot.remove(i);
            }
        }
        if doc["override"]
            .as_array_of_tables()
            .is_some_and(|a| a.is_empty())
        {
            doc.remove("override");
        }
    }
    write_doc(&file, &doc)?;
    println!(
        "{rel} : surcharge « {key} » retirée.{}",
        if empty {
            " Plus aucune surcharge : le fichier sera relié au dépôt au prochain setup (un fichier généré qui diffère sera un conflit à trancher)."
        } else {
            ""
        }
    );
    Ok(())
}

/// `poste sys override` : les surcharges et l'état des fichiers générés.
pub fn override_list(env: &Env, repo: &Path) -> Result<()> {
    let sets = config::override_sets(env)?;
    if sets.is_empty() {
        println!("Aucune surcharge (poste sys override set <chemin> <clé> <valeur>).");
        return Ok(());
    }
    let layout = config::load(repo, env)?;
    println!("Surcharges de cette machine (config.toml, [[override]])");
    for (rel, s) in &sets {
        let state = match layout
            .as_ref()
            .and_then(|l| l.entries.iter().find(|e| e.rel == *rel))
        {
            Some(e) => config::describe(&config::state_of(e, env)?),
            None => "fichier local ou inconnu du dépôt".into(),
        };
        println!("  ~/{rel}  [{state}]");
        for (k, v) in s {
            println!("      {k} = {}", ojson::pretty(v).trim());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- modules

/// `poste sys module` : modules du dépôt, activés ou non, installés ou non.
pub fn module_list(env: &Env, repo: &Path) -> Result<()> {
    let cfg = Config::load(env)?;
    let mods = plan::repo_modules(repo)?;
    println!("Modules du dépôt (config.toml, clé disabled : ceux qu'on n'installe pas ici)");
    for m in &mods {
        let off = m.name != "sys" && cfg.is_disabled(&m.name);
        let installed = env.modules_dir().join(&m.name).is_dir();
        let state = match (off, installed) {
            (true, true) => "désactivé, encore installé (poste sys setup le retire)",
            (true, false) => "désactivé",
            (false, true) => "activé, installé",
            (false, false) => "activé, pas encore installé",
        };
        println!("  {:<10} {state}", m.name);
    }
    for d in cfg
        .disabled
        .iter()
        .filter(|d| !mods.iter().any(|m| &m.name == *d))
    {
        println!("  {d:<10} inconnu du dépôt (entrée inutile de disabled)");
    }
    Ok(())
}

/// `poste sys module enable|disable <nom>`.
pub fn module_set(env: &Env, repo: &Path, name: &str, enable: bool) -> Result<()> {
    let file = env.config_file();
    if enable {
        if !edit_list(&file, "disabled", name, false)? {
            println!("{name} : n'était pas désactivé.");
            return Ok(());
        }
        println!("{name} : réactivé. Le prochain `poste sys setup` l'installe.");
        return Ok(());
    }
    if name == "sys" {
        bail!("sys ne peut pas être désactivé : il gère l'installation");
    }
    if !plan::repo_modules(repo)?.iter().any(|m| m.name == name) {
        bail!("{name} n'est pas un module du dépôt (poste sys module pour la liste)");
    }
    let changed = edit_list(&file, "disabled", name, true)?;
    println!(
        "{name} : {} désactivé ({}).",
        if changed { "" } else { "déjà" },
        env.display(&file)
    );
    if env.modules_dir().join(name).is_dir() {
        println!("  Il est installé : `poste sys setup` en montrera le retrait (fichiers, hypr/, island/, lignes posées, unités) avec aperçu, avant de demander confirmation.");
    }
    Ok(())
}

// ---------------------------------------------------------------- skip

/// `poste sys skip` : identifiants ignorés et ce qu'ils font sauter.
pub fn skip_list(env: &Env, repo: &Path) -> Result<()> {
    let cfg = Config::load(env)?;
    if cfg.skip.is_empty() {
        println!("Rien d'ignoré : tout le bootstrap s'applique (poste sys skip add <paquet ou id d'étape>…).");
        return Ok(());
    }
    let catalog = provision::skip_catalog(env, repo)?;
    println!("Ignorés sur cette machine (config.toml, clé skip)");
    for id in &cfg.skip {
        match catalog.get(id) {
            Some(kind) => println!("  {id:<40} {kind}"),
            None => println!("  {id:<40} inconnu du bootstrap (faute de frappe ?)"),
        }
    }
    let eff = provision::skip_effect(env, repo, &cfg.skip)?;
    let deps: Vec<&String> = eff
        .ignored
        .iter()
        .filter(|i| !cfg.skip.contains(i))
        .collect();
    if !deps.is_empty() {
        println!(
            "  sautées avec eux (elles en dépendent) : {}",
            deps.iter()
                .map(|d| d.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

/// `poste sys skip add <id>…` : un identifiant inconnu est ajouté quand même, avec un avertissement.
pub fn skip_add(env: &Env, repo: &Path, ids: &[String]) -> Result<()> {
    let file = env.config_file();
    let catalog = provision::skip_catalog(env, repo)?;
    for id in ids {
        let known = catalog.get(id);
        if known.is_none() {
            eprintln!("avertissement : « {id} » n'est ni un paquet de packages.toml ni l'id d'une étape de steps.toml (faute de frappe ?)");
        }
        let changed = edit_list(&file, "skip", id, true)?;
        let tag = if changed {
            "ignoré ici"
        } else {
            "déjà ignoré"
        };
        match known {
            Some(kind) => println!("{id} : {tag} ({kind}, {}).", env.display(&file)),
            None => println!("{id} : {tag} ({}).", env.display(&file)),
        }
    }
    println!("  setup et status ne l'exigent, ne le proposent ni ne le comptent plus manquant. Rien n'est désinstallé.");
    Ok(())
}

/// `poste sys skip remove <id>…` : les identifiants absents de `skip` font échouer la commande,
/// après avoir retiré les autres.
pub fn skip_remove(env: &Env, ids: &[String]) -> Result<()> {
    let file = env.config_file();
    let mut absent = Vec::new();
    for id in ids {
        if edit_list(&file, "skip", id, false)? {
            println!("{id} : n'est plus ignoré.");
        } else {
            absent.push(id.as_str());
        }
    }
    if !absent.is_empty() {
        bail!(
            "pas dans skip ({}) : {}",
            env.display(&file),
            absent.join(", ")
        );
    }
    println!("  Le prochain setup le remet au bootstrap.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn edition_conserve_le_reste() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("poste/config.toml");
        // Fichier absent : créé.
        assert!(edit_list(&f, "disabled", "record", true).unwrap());
        fs::write(
            &f,
            "# ma config\nrepo = \"/r\"\n\ndisabled = [\"record\"]\n",
        )
        .unwrap();
        assert!(!edit_list(&f, "disabled", "record", true).unwrap());
        assert!(edit_list(&f, "disabled", "power", true).unwrap());
        assert!(edit_list(&f, "local", ".config/a", true).unwrap());
        let t = fs::read_to_string(&f).unwrap();
        assert!(t.contains("# ma config") && t.contains("repo = \"/r\""));
        let c = Config::parse(&t).unwrap();
        assert_eq!(c.disabled, ["record", "power"]);
        assert_eq!(c.local, [".config/a"]);
        assert!(edit_list(&f, "disabled", "record", false).unwrap());
        assert!(!edit_list(&f, "disabled", "record", false).unwrap());
        let c = Config::parse(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(c.disabled, ["power"]);
    }

    #[test]
    fn cle_source_ecrite_sans_toucher_au_reste() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("poste/config.toml");
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(
            &f,
            "# ma config\nrepo = \"/r\"\n\n[[override]]\npath = \".x\"\n",
        )
        .unwrap();
        assert!(set_key(&f, "source", "release").unwrap());
        assert!(!set_key(&f, "source", "release").unwrap());
        let t = fs::read_to_string(&f).unwrap();
        assert!(t.contains("# ma config") && t.contains("[[override]]"));
        let c = Config::parse(&t).unwrap();
        assert_eq!(c.source, poste_core::Source::Release);
        assert_eq!(c.overrides.len(), 1);
        assert!(set_key(&f, "source", "dev").unwrap());
        let c = Config::parse(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(c.source, poste_core::Source::Dev);
    }

    #[test]
    fn liste_invalide_refusee() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("config.toml");
        fs::write(&f, "disabled = 3\n").unwrap();
        assert!(edit_list(&f, "disabled", "x", true).is_err());
    }

    fn repo_et_home() -> (tempfile::TempDir, std::path::PathBuf, Env) {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir_all(repo.join("config/.config/hypr")).unwrap();
        fs::write(repo.join("config/.config/hypr/monitors.lua"), "m").unwrap();
        fs::create_dir_all(repo.join("modules/record")).unwrap();
        fs::write(
            repo.join("modules/record/module.toml"),
            "name = \"record\"\n",
        )
        .unwrap();
        fs::create_dir_all(repo.join("modules/sys")).unwrap();
        fs::write(repo.join("modules/sys/module.toml"), "name = \"sys\"\n").unwrap();
        let home = d.path().join("home");
        fs::create_dir_all(&home).unwrap();
        (d, repo, Env::with_home(&home))
    }

    #[test]
    fn local_add_remove_validations() {
        let (_d, repo, env) = repo_et_home();
        assert!(local_add(&env, &repo, ".config/hypr/inconnu.lua").is_err());
        local_add(&env, &repo, "~/.config/hypr/monitors.lua").unwrap();
        local_add(&env, &repo, ".config/hypr/monitors.lua").unwrap(); // idempotent
        let c = Config::load(&env).unwrap();
        assert_eq!(c.local, [".config/hypr/monitors.lua"]);
        assert!(local_remove(&env, ".config/hypr/autre").is_err());
        local_remove(&env, ".config/hypr/monitors.lua").unwrap();
        assert!(Config::load(&env).unwrap().local.is_empty());
    }

    #[test]
    fn module_disable_validations() {
        let (_d, repo, env) = repo_et_home();
        assert!(module_set(&env, &repo, "sys", false).is_err());
        assert!(module_set(&env, &repo, "nope", false).is_err());
        module_set(&env, &repo, "record", false).unwrap();
        assert_eq!(Config::load(&env).unwrap().disabled, ["record"]);
        module_set(&env, &repo, "record", true).unwrap();
        assert!(Config::load(&env).unwrap().disabled.is_empty());
    }

    #[test]
    fn skip_add_remove_list() {
        let (_d, repo, env) = repo_et_home();
        fs::create_dir_all(repo.join("bootstrap")).unwrap();
        fs::write(
            repo.join("bootstrap/packages.toml"),
            "[packages]\ngimp = \"x\"\n",
        )
        .unwrap();
        fs::write(
            repo.join("bootstrap/steps.toml"),
            "[[manual]]\nid = \"m1\"\ntitle = \"t\"\n",
        )
        .unwrap();
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "# perso\nrepo = \"/r\"\n").unwrap();
        skip_add(&env, &repo, &["gimp".into(), "m1".into(), "typo".into()]).unwrap();
        skip_add(&env, &repo, &["gimp".into()]).unwrap(); // idempotent
        let t = fs::read_to_string(env.config_file()).unwrap();
        assert!(t.contains("# perso") && t.contains("repo = \"/r\""));
        assert_eq!(Config::parse(&t).unwrap().skip, ["gimp", "m1", "typo"]);
        skip_list(&env, &repo).unwrap();
        assert!(skip_remove(&env, &["gimp".into(), "absent".into()]).is_err());
        assert_eq!(Config::load(&env).unwrap().skip, ["m1", "typo"]);
        skip_remove(&env, &["m1".into(), "typo".into()]).unwrap();
        assert!(Config::load(&env).unwrap().skip.is_empty());
    }

    #[test]
    fn override_set_unset_cycle() {
        let (_d, repo, env) = repo_et_home();
        fs::write(
            repo.join("config/.config/hypr/s.json"),
            "{\"idle\": {\"lock\": 5400}}",
        )
        .unwrap();
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "# perso\nrepo = \"/r\"\n").unwrap();
        assert!(override_set(&env, &repo, ".config/hypr/inconnu.json", "a", "1").is_err());
        assert!(override_set(&env, &repo, ".config/hypr/s.json", "idle.lock.x", "1").is_err());
        override_set(&env, &repo, ".config/hypr/s.json", "idle.lock", "3600").unwrap();
        override_set(&env, &repo, ".config/hypr/s.json", "idle.name", "soir").unwrap();
        let t = fs::read_to_string(env.config_file()).unwrap();
        assert!(t.contains("# perso"));
        let c = Config::parse(&t).unwrap();
        assert_eq!(c.overrides.len(), 1);
        let sets = crate::overrides::sets_of(&c.overrides[0]).unwrap();
        assert_eq!(sets.len(), 2);
        assert!(sets.contains(&("idle.name".into(), J::Str("soir".into()))));
        override_unset(&env, ".config/hypr/s.json", "idle.lock").unwrap();
        assert!(override_unset(&env, ".config/hypr/s.json", "idle.lock").is_err());
        override_unset(&env, ".config/hypr/s.json", "idle.name").unwrap();
        let c = Config::load(&env).unwrap();
        assert!(c.overrides.is_empty());
        assert!(fs::read_to_string(env.config_file())
            .unwrap()
            .contains("# perso"));
    }
}
