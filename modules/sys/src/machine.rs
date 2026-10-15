//! Settings specific to this machine, in `~/.config/poste/config.toml` (never in the repository):
//! `local` (files of `config/` kept to the machine) and `disabled` (modules not installed).
//! `poste sys local …` and `poste sys module …` edit these two lists without touching the rest
//! of the file (comments and other keys kept).

use crate::config;
use crate::ojson::{self, J};
use crate::plan;
use crate::provision;
use anyhow::{bail, Context, Result};
use poste_core::{Config, Env};
use std::path::Path;
use toml_edit::{Array, ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

/// Adds (`add = true`) or removes `value` from the list `key` of the file. Returns `true` if the
/// file changed. The file is created if needed; atomic write.
pub fn edit_list(file: &Path, key: &str, value: &str, add: bool) -> Result<bool> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", file.display())),
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
            bail!("{}: '{key}' is not a list", file.display());
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

/// Sets the top-level key `key = "value"` (string) without touching the rest of the file.
/// Returns `true` if the file changed. The file is created if needed; atomic write.
pub fn set_key(file: &Path, key: &str, value: &str) -> Result<bool> {
    let text = match std::fs::read_to_string(file) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e).with_context(|| format!("reading {}", file.display())),
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

/// `poste sys local`: lists the local files.
pub fn local_list(env: &Env, repo: &Path) -> Result<()> {
    let Some(layout) = config::load(repo, env)? else {
        bail!("no config/ folder in the repository");
    };
    if layout.local_paths.is_empty() {
        println!("No local file: all of config/ is linked (poste sys local add <path> to keep one to this machine).");
        return Ok(());
    }
    config::print_local(&layout, env)
}

/// `poste sys local add <path>`.
pub fn local_add(env: &Env, repo: &Path, arg: &str) -> Result<()> {
    let rel = config::rel_of(env, arg)?;
    let Some(all) = config::load_with(repo, env, &[], &[])? else {
        bail!("no config/ folder in the repository");
    };
    if let Some(d) = all
        .link_dirs
        .iter()
        .find(|d| rel.starts_with(&format!("{d}/")))
    {
        bail!("{rel} is in the folder linked as a block '{d}': declare that folder as local");
    }
    if !all
        .entries
        .iter()
        .any(|e| config::is_local(&e.rel, std::slice::from_ref(&rel)))
    {
        bail!("{rel} is not an entry of the config/ of the repository (nothing to keep local)");
    }
    let changed = edit_list(&env.config_file(), "local", &rel, true)?;
    let tag = if changed { "added to" } else { "already in" };
    println!("{rel}: {tag} local ({}).", env.display(&env.config_file()));
    // Current state of the target, to say what to do with the existing link.
    for e in all
        .entries
        .iter()
        .filter(|e| config::is_local(&e.rel, std::slice::from_ref(&rel)))
    {
        if config::state_of(e, env)? == config::State::Linked {
            println!(
                "  {} is still a link to the repository: poste will no longer touch it, but editing it would change the repository. To make it an ordinary file (copy), run `poste sys forget {}`.",
                env.display(&e.dest),
                e.rel
            );
        }
    }
    println!(
        "  setup no longer links it, no longer reports it as a conflict and never touches it."
    );
    Ok(())
}

/// `poste sys local remove <path>`.
pub fn local_remove(env: &Env, arg: &str) -> Result<()> {
    let rel = config::rel_of(env, arg)?;
    if !edit_list(&env.config_file(), "local", &rel, false)? {
        bail!(
            "{rel} is not in local ({})",
            env.display(&env.config_file())
        );
    }
    println!("{rel}: removed from local.");
    println!("  The next setup will link it to the repository; if the file in ~ is ordinary and differs, it will be a conflict to decide (poste sys diff {rel}).");
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
        J::Null => bail!("null cannot be an override (TOML has no null)"),
        J::Obj(_) => {
            bail!("give the keys one by one (idle.lock, idle.screensaver…) rather than an object")
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
        Err(e) => return Err(e).with_context(|| format!("reading {}", file.display())),
    };
    text.parse()
        .with_context(|| format!("{} invalide", file.display()))
}

fn check_key(key: &str) -> Result<()> {
    if key.is_empty() || key.split('.').any(str::is_empty) {
        bail!("invalid key: '{key}' (expected: a.b.c)");
    }
    Ok(())
}

/// `poste sys override set <path> <key> <value>`: the value is JSON (3600, true, "text",
/// [1,2]); what is not JSON becomes a string.
pub fn override_set(env: &Env, repo: &Path, arg: &str, key: &str, value: &str) -> Result<()> {
    check_key(key)?;
    let rel = config::rel_of(env, arg)?;
    let Some(all) = config::load_with(repo, env, &[], &[])? else {
        bail!("no config/ folder in the repository");
    };
    let Some(e) = all.entries.iter().find(|e| e.rel == rel) else {
        bail!("{rel} is not a config/ file of the repository");
    };
    let repo_text = std::fs::read_to_string(&e.src)?;
    ojson::parse(&repo_text)
        .with_context(|| format!("{rel} is not valid JSON in the repository"))?;
    let j = ojson::parse(value).unwrap_or_else(|_| J::Str(value.to_string()));
    let tv = j_to_toml(&j)?;
    // The result must stay generable (key going through a non-object value: refused).
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
                "{}: 'override' must be a list of [[override]]",
                file.display()
            );
        };
        let mut t = Table::new();
        t["path"] = Item::Value(Value::from(rel.as_str()));
        t["set"] = Item::Value(Value::InlineTable(InlineTable::new()));
        aot.push(t);
    }
    let t = find_override(&mut doc, &rel).expect("table added");
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
        _ => bail!("{}: 'set' must be a table", file.display()),
    }
    write_doc(&file, &doc)?;
    println!(
        "{rel}: {key} = {} (override of this machine, {}).",
        ojson::pretty(&j).trim(),
        env.display(&file)
    );
    println!("  The file will be generated (repository + overrides) at the next `poste sys setup`; it is no longer a link.");
    Ok(())
}

/// `poste sys override unset <path> <key>`.
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
        bail!("{rel}: no override '{key}' ({})", env.display(&file));
    }
    // No key left: the table disappears and the file becomes a link again at the next setup.
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
        "{rel}: override '{key}' removed.{}",
        if empty {
            " No override left: the file will be linked to the repository again at the next setup (a generated file that differs will be a conflict to decide)."
        } else {
            ""
        }
    );
    Ok(())
}

/// `poste sys override`: the overrides and the state of the generated files.
pub fn override_list(env: &Env, repo: &Path) -> Result<()> {
    let sets = config::override_sets(env)?;
    if sets.is_empty() {
        println!("No override (poste sys override set <path> <key> <value>).");
        return Ok(());
    }
    let layout = config::load(repo, env)?;
    println!("Overrides of this machine (config.toml, [[override]])");
    for (rel, s) in &sets {
        let state = match layout
            .as_ref()
            .and_then(|l| l.entries.iter().find(|e| e.rel == *rel))
        {
            Some(e) => config::describe(&config::state_of(e, env)?),
            None => "local file or unknown to the repository".into(),
        };
        println!("  ~/{rel}  [{state}]");
        for (k, v) in s {
            println!("      {k} = {}", ojson::pretty(v).trim());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- modules

/// `poste sys module`: modules of the repository, enabled or not, installed or not.
pub fn module_list(env: &Env, repo: &Path) -> Result<()> {
    let cfg = Config::load(env)?;
    let mods = plan::repo_modules(repo)?;
    println!("Modules of the repository (config.toml, key disabled: the ones not installed here)");
    for m in &mods {
        let off = m.name != "sys" && cfg.is_disabled(&m.name);
        let installed = env.modules_dir().join(&m.name).is_dir();
        if let (false, Some(prog)) = (off, m.missing(env)) {
            let more = if installed {
                ", still installed (poste sys setup removes it)"
            } else {
                ""
            };
            println!("  {:<10} {prog} not found: left out{more}", m.name);
            continue;
        }
        let state = match (off, installed) {
            (true, true) => "disabled, still installed (poste sys setup removes it)",
            (true, false) => "disabled",
            (false, true) => "enabled, installed",
            (false, false) => "enabled, not installed yet",
        };
        println!("  {:<10} {state}", m.name);
    }
    for d in cfg
        .disabled
        .iter()
        .filter(|d| !mods.iter().any(|m| &m.name == *d))
    {
        println!("  {d:<10} unknown to the repository (useless entry in disabled)");
    }
    Ok(())
}

/// `poste sys addons`: the add-ons of the repository, chosen or not, installed or not.
pub fn addon_list(env: &Env, repo: &Path) -> Result<()> {
    let cfg = Config::load(env)?;
    let addons = plan::repo_addons(repo)?;
    println!("Add-ons (key addons, in poste.toml of the data repository)");
    for a in &addons {
        let chosen = cfg.addons.contains(&a.name);
        let installed = env.modules_dir().join(&a.name).is_dir();
        let state = match (chosen, a.missing(env), installed) {
            (false, _, true) => "not chosen, still installed (poste sys setup removes it)".into(),
            (false, _, false) => "available".into(),
            (true, Some(p), true) => {
                format!("chosen, {p} not found: still plugged in (poste sys setup removes it)")
            }
            (true, Some(p), false) => format!("chosen, {p} not installed yet (poste sys setup)"),
            (true, None, true) => "chosen, plugged in".into(),
            (true, None, false) => "chosen, not plugged in yet (poste sys setup)".into(),
        };
        println!("  {:<10} {state}", a.name);
        if !a.description.is_empty() {
            println!("  {:<10} {}", "", a.description);
        }
    }
    for name in cfg
        .addons
        .iter()
        .filter(|n| !addons.iter().any(|a| &a.name == *n))
    {
        println!("  {name:<10} unknown to the repository (useless entry in addons)");
    }
    Ok(())
}

/// `poste sys module enable|disable <name>`.
pub fn module_set(env: &Env, repo: &Path, name: &str, enable: bool) -> Result<()> {
    let file = env.config_file();
    if enable {
        if !edit_list(&file, "disabled", name, false)? {
            println!("{name}: was not disabled.");
            return Ok(());
        }
        println!("{name}: enabled again. The next `poste sys setup` installs it.");
        return Ok(());
    }
    if name == "sys" {
        bail!("sys cannot be disabled: it manages the installation");
    }
    if !plan::repo_modules(repo)?.iter().any(|m| m.name == name) {
        bail!("{name} is not a module of the repository (poste sys module for the list)");
    }
    let changed = edit_list(&file, "disabled", name, true)?;
    println!(
        "{name}: {} disabled ({}).",
        if changed { "" } else { "already" },
        env.display(&file)
    );
    if env.modules_dir().join(name).is_dir() {
        println!("  It is installed: `poste sys setup` will show its removal (files, hypr/, island/, lines put in place, units) with a preview, before asking for confirmation.");
    }
    Ok(())
}

// ---------------------------------------------------------------- skip

/// `poste sys skip`: ignored identifiers and what they make skipped.
pub fn skip_list(env: &Env, repo: &Path) -> Result<()> {
    let cfg = Config::load(env)?;
    if cfg.skip.is_empty() {
        println!("Nothing ignored: all the bootstrap applies (poste sys skip add <package or step id>…).");
        return Ok(());
    }
    let catalog = provision::skip_catalog(env, repo)?;
    println!("Ignored on this machine (config.toml, key skip)");
    for id in &cfg.skip {
        match catalog.get(id) {
            Some(kind) => println!("  {id:<40} {kind}"),
            None => println!("  {id:<40} unknown to the bootstrap (typo?)"),
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
            "  skipped with them (they depend on them): {}",
            deps.iter()
                .map(|d| d.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

/// `poste sys skip add <id>…`: an unknown identifier is added anyway, with a warning.
pub fn skip_add(env: &Env, repo: &Path, ids: &[String]) -> Result<()> {
    let file = env.config_file();
    let catalog = provision::skip_catalog(env, repo)?;
    for id in ids {
        let known = catalog.get(id);
        if known.is_none() {
            eprintln!("warning: '{id}' is neither a package of packages.toml nor the id of a step of steps.toml (typo?)");
        }
        let changed = edit_list(&file, "skip", id, true)?;
        let tag = if changed {
            "ignored here"
        } else {
            "already ignored"
        };
        match known {
            Some(kind) => println!("{id}: {tag} ({kind}, {}).", env.display(&file)),
            None => println!("{id}: {tag} ({}).", env.display(&file)),
        }
    }
    println!("  setup and status no longer require it, offer it or count it as missing. Nothing is uninstalled.");
    Ok(())
}

/// `poste sys skip remove <id>…`: identifiers absent from `skip` make the command fail,
/// after removing the others.
pub fn skip_remove(env: &Env, ids: &[String]) -> Result<()> {
    let file = env.config_file();
    let mut absent = Vec::new();
    for id in ids {
        if edit_list(&file, "skip", id, false)? {
            println!("{id}: no longer ignored.");
        } else {
            absent.push(id.as_str());
        }
    }
    if !absent.is_empty() {
        bail!(
            "not in skip ({}): {}",
            env.display(&file),
            absent.join(", ")
        );
    }
    println!("  The next setup puts it back into the bootstrap.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn edit_keeps_the_rest() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("poste/config.toml");
        // Missing file: created.
        assert!(edit_list(&f, "disabled", "record", true).unwrap());
        fs::write(
            &f,
            "# my config\nrepo = \"/r\"\n\ndisabled = [\"record\"]\n",
        )
        .unwrap();
        assert!(!edit_list(&f, "disabled", "record", true).unwrap());
        assert!(edit_list(&f, "disabled", "power", true).unwrap());
        assert!(edit_list(&f, "local", ".config/a", true).unwrap());
        let t = fs::read_to_string(&f).unwrap();
        assert!(t.contains("# my config") && t.contains("repo = \"/r\""));
        let c = Config::parse(&t).unwrap();
        assert_eq!(c.disabled, ["record", "power"]);
        assert_eq!(c.local, [".config/a"]);
        assert!(edit_list(&f, "disabled", "record", false).unwrap());
        assert!(!edit_list(&f, "disabled", "record", false).unwrap());
        let c = Config::parse(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(c.disabled, ["power"]);
    }

    #[test]
    fn source_key_written_without_touching_the_rest() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("poste/config.toml");
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(
            &f,
            "# my config\nrepo = \"/r\"\n\n[[override]]\npath = \".x\"\n",
        )
        .unwrap();
        assert!(set_key(&f, "source", "release").unwrap());
        assert!(!set_key(&f, "source", "release").unwrap());
        let t = fs::read_to_string(&f).unwrap();
        assert!(t.contains("# my config") && t.contains("[[override]]"));
        let c = Config::parse(&t).unwrap();
        assert_eq!(c.source, poste_core::Source::Release);
        assert_eq!(c.overrides.len(), 1);
        assert!(set_key(&f, "source", "dev").unwrap());
        let c = Config::parse(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(c.source, poste_core::Source::Dev);
    }

    #[test]
    fn invalid_list_refused() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("config.toml");
        fs::write(&f, "disabled = 3\n").unwrap();
        assert!(edit_list(&f, "disabled", "x", true).is_err());
    }

    fn repo_and_home() -> (tempfile::TempDir, std::path::PathBuf, Env) {
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
        let (_d, repo, env) = repo_and_home();
        assert!(local_add(&env, &repo, ".config/hypr/inconnu.lua").is_err());
        local_add(&env, &repo, "~/.config/hypr/monitors.lua").unwrap();
        local_add(&env, &repo, ".config/hypr/monitors.lua").unwrap(); // idempotent
        let c = Config::load(&env).unwrap();
        assert_eq!(c.local, [".config/hypr/monitors.lua"]);
        assert!(local_remove(&env, ".config/hypr/other").is_err());
        local_remove(&env, ".config/hypr/monitors.lua").unwrap();
        assert!(Config::load(&env).unwrap().local.is_empty());
    }

    #[test]
    fn module_disable_validations() {
        let (_d, repo, env) = repo_and_home();
        assert!(module_set(&env, &repo, "sys", false).is_err());
        assert!(module_set(&env, &repo, "nope", false).is_err());
        module_set(&env, &repo, "record", false).unwrap();
        assert_eq!(Config::load(&env).unwrap().disabled, ["record"]);
        module_set(&env, &repo, "record", true).unwrap();
        assert!(Config::load(&env).unwrap().disabled.is_empty());
    }

    #[test]
    fn skip_add_remove_list() {
        let (_d, repo, env) = repo_and_home();
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
        let (_d, repo, env) = repo_and_home();
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
