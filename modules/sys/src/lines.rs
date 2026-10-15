//! Lines that modules put in files without includes.
//!
//! A module declares `[[line]] file = "…", line = "…"` in `module.toml`. `setup` adds
//! the line if it is missing and notes the owner module in `installed.json`; it is
//! removed if the module no longer declares it (or disappears). A line that was not put in place
//! by us is never changed or removed. The repository can also declare "base" lines
//! in `config/.poste/lines.toml` (same format, owner `config`).
//!
//! The key of a line is what precedes the first `=` (or the whole line): two modules
//! that want the same key in the same file are in conflict, as is a key already
//! present with another value that we did not put in place.

use crate::manifest::{LineEntry, Manifest};
use crate::plan;
use anyhow::{bail, Context, Result};
use poste_core::{time, Card, Env};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Owner of the lines of `config/.poste/lines.toml`.
pub const BASE_OWNER: &str = "config";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub module: String,
    pub file: PathBuf,
    pub line: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Adds the line at the end of the file.
    Add,
    /// Replaces the old value that we had put in place.
    Replace { old: String },
    /// Put in place by us and present.
    Unchanged,
    /// The same line already exists without us having put it in place: left, not tracked.
    Foreign,
    /// Nothing is written; reason shown.
    Conflict(String),
    /// Put in place by us, no longer declared: removed.
    Remove,
    /// Put in place by us but already gone from the file: only the entry is forgotten.
    Forget,
    /// File that we do not modify (symbolic link, unreadable).
    Blocked(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub op: Op,
    pub module: String,
    pub file: PathBuf,
    pub line: String,
}

impl Step {
    pub fn writes(&self) -> bool {
        matches!(
            self.op,
            Op::Add | Op::Replace { .. } | Op::Remove | Op::Forget
        )
    }
}

/// Key of a line: what precedes the first `=`, otherwise the whole line.
pub fn key_of(line: &str) -> String {
    let t = line.trim();
    match t.split_once('=') {
        Some((k, _)) => k.trim().to_string(),
        None => t.to_string(),
    }
}

fn is_comment(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with('#') || t.starts_with("//") || t.starts_with("--")
}

fn resolve_file(env: &Env, owner: &str, s: &str) -> Result<PathBuf> {
    let p = env.expand(s);
    if !p.is_absolute()
        || !p.starts_with(&env.home)
        || p.components().any(|c| c.as_os_str() == "..")
    {
        bail!("{owner}: the file '{s}' must be under ~ (without '..')");
    }
    Ok(p)
}

fn specs_of(card: &Card, owner: &str, env: &Env, out: &mut Vec<Spec>) -> Result<()> {
    for l in &card.line {
        let line = l
            .line
            .replace("{modules}", &env.modules_dir().to_string_lossy())
            .replace("{home}", &env.home.to_string_lossy())
            .trim()
            .to_string();
        if line.is_empty() || line.contains('\n') {
            bail!("{owner}: a [[line]] must fit on one non-empty line");
        }
        let spec = Spec {
            module: owner.to_string(),
            file: resolve_file(env, owner, &l.file)?,
            key: key_of(&line),
            line,
        };
        if !out.contains(&spec) {
            out.push(spec);
        }
    }
    Ok(())
}

/// The wanted lines: those of the modules of the repository, then those of `config/.poste/lines.toml`.
pub fn desired(repo: &Path, env: &Env) -> Result<Vec<Spec>> {
    let mut out = Vec::new();
    for m in plan::enabled_modules(repo, env)? {
        let card = Card::load(&m.dir.join("module.toml"))?;
        specs_of(&card, &m.name, env, &mut out)?;
    }
    let base = repo.join("config/.poste/lines.toml");
    if base.is_file() {
        specs_of(&Card::load(&base)?, BASE_OWNER, env, &mut out)?;
    }
    Ok(out)
}

enum Content {
    /// Missing file (or about to disappear).
    Missing,
    Text(String),
    Blocked(String),
}

fn read(file: &Path, vanishing: &HashSet<PathBuf>) -> Content {
    if vanishing.contains(file) {
        return Content::Missing;
    }
    match std::fs::symlink_metadata(file) {
        Err(_) => Content::Missing,
        Ok(m) if m.file_type().is_symlink() => Content::Blocked(
            "the file is a symbolic link (writing through it would change its target)".into(),
        ),
        Ok(m) if !m.is_file() => Content::Blocked("it is not an ordinary file".into()),
        Ok(_) => match std::fs::read_to_string(file) {
            Ok(t) => Content::Text(t),
            Err(e) => Content::Blocked(format!("illisible : {e}")),
        },
    }
}

/// Computes what must be done. `vanishing`: files about to be removed (orphan
/// links), seen as absent.
pub fn plan(specs: &[Spec], manifest: &Manifest, vanishing: &HashSet<PathBuf>) -> Vec<Step> {
    let mut steps = Vec::new();
    // Same (file, key) wanted by several owners or values: conflict for all.
    let mut by_key: BTreeMap<(&Path, &str), Vec<&Spec>> = BTreeMap::new();
    for s in specs {
        by_key.entry((&s.file, &s.key)).or_default().push(s);
    }
    let owned = |file: &Path, module: &str, key: &str| {
        manifest
            .lines
            .iter()
            .find(|e| Path::new(&e.file) == file && e.module == module && key_of(&e.line) == key)
    };
    let mut cache: BTreeMap<PathBuf, Content> = BTreeMap::new();

    for s in specs {
        let step = |op| Step {
            op,
            module: s.module.clone(),
            file: s.file.clone(),
            line: s.line.clone(),
        };
        let rivals = &by_key[&(s.file.as_path(), s.key.as_str())];
        if rivals.len() > 1 {
            let others: Vec<&str> = rivals
                .iter()
                .filter(|r| **r != s)
                .map(|r| r.module.as_str())
                .collect();
            steps.push(step(Op::Conflict(format!(
                "the key '{}' is also asked for by {}",
                s.key,
                others.join(", ")
            ))));
            continue;
        }
        let content = cache
            .entry(s.file.clone())
            .or_insert_with(|| read(&s.file, vanishing));
        let text = match content {
            Content::Blocked(why) => {
                steps.push(step(Op::Blocked(why.clone())));
                continue;
            }
            Content::Missing => "",
            Content::Text(t) => t.as_str(),
        };
        let lines: Vec<&str> = text.lines().collect();
        if lines.iter().any(|l| l.trim() == s.line) {
            let ours = manifest
                .lines
                .iter()
                .any(|e| Path::new(&e.file) == s.file && e.module == s.module && e.line == s.line);
            steps.push(step(if ours { Op::Unchanged } else { Op::Foreign }));
            continue;
        }
        let same_key = lines
            .iter()
            .find(|l| !is_comment(l) && key_of(l) == s.key && !l.trim().is_empty());
        match (same_key, owned(&s.file, &s.module, &s.key)) {
            (Some(l), Some(e)) if e.line == l.trim() => steps.push(step(Op::Replace {
                old: e.line.clone(),
            })),
            (Some(l), _) => steps.push(step(Op::Conflict(format!(
                "the key already exists with another value that we did not put in place: {}",
                l.trim()
            )))),
            (None, _) => steps.push(step(Op::Add)),
        }
    }

    // Lines that we had put in place and that nobody declares any more.
    for e in &manifest.lines {
        let file = PathBuf::from(&e.file);
        let key = key_of(&e.line);
        if specs
            .iter()
            .any(|s| s.file == file && s.module == e.module && s.key == key)
        {
            continue;
        }
        let step = |op| Step {
            op,
            module: e.module.clone(),
            file: file.clone(),
            line: e.line.clone(),
        };
        let content = cache
            .entry(file.clone())
            .or_insert_with(|| read(&file, vanishing));
        match content {
            Content::Blocked(why) => steps.push(step(Op::Blocked(why.clone()))),
            Content::Missing => steps.push(step(Op::Forget)),
            Content::Text(t) => steps.push(step(if t.lines().any(|l| l.trim() == e.line) {
                Op::Remove
            } else {
                Op::Forget
            })),
        }
    }
    steps
}

pub fn label(op: &Op) -> &'static str {
    match op {
        Op::Add => "added",
        Op::Replace { .. } => "replaced",
        Op::Unchanged => "in place",
        Op::Foreign => "already there (not put by poste, left)",
        Op::Conflict(_) => "CONFLICT",
        Op::Remove => "removed",
        Op::Forget => "already absent (forgotten)",
        Op::Blocked(_) => "NOT MODIFIED",
    }
}

pub fn print_plan(env: &Env, steps: &[Step]) {
    let shown: Vec<&Step> = steps
        .iter()
        .filter(|s| !matches!(s.op, Op::Unchanged))
        .collect();
    if shown.is_empty() {
        return;
    }
    println!("\nModule lines:");
    for s in shown {
        println!(
            "  {:<12} {}  {}  [{}]",
            label(&s.op),
            env.display(&s.file),
            s.line,
            s.module
        );
        match &s.op {
            Op::Conflict(why) | Op::Blocked(why) => println!("               {why}"),
            Op::Replace { old } => println!("               (old value: {old})"),
            _ => {}
        }
    }
}

pub fn conflicts(steps: &[Step]) -> usize {
    steps
        .iter()
        .filter(|s| matches!(s.op, Op::Conflict(_) | Op::Blocked(_)))
        .count()
}

/// Writes `text` into `file` (neighbouring temporary file then rename), same permissions as before.
fn write_atomic(file: &Path, text: &str) -> Result<()> {
    let parent = file.parent().context("file without a parent folder")?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".poste-tmp-{}-line", std::process::id()));
    std::fs::write(&tmp, text)?;
    if let Ok(m) = std::fs::metadata(file) {
        let _ = std::fs::set_permissions(&tmp, m.permissions());
    }
    if let Err(e) = std::fs::rename(&tmp, file) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("writing {}", file.display()));
    }
    Ok(())
}

fn remove_first(lines: &mut Vec<String>, wanted: &str) -> bool {
    match lines.iter().position(|l| l.trim() == wanted) {
        Some(i) => {
            lines.remove(i);
            true
        }
        None => false,
    }
}

/// Applies the steps that write; returns the number of lines handled. Backs up an
/// existing file before changing it. Updates `manifest` (to be saved by the caller).
pub fn apply(env: &Env, steps: &[Step], stamp: &str, manifest: &mut Manifest) -> Result<usize> {
    let mut files: Vec<&Path> = Vec::new();
    for s in steps.iter().filter(|s| s.writes()) {
        if !files.contains(&s.file.as_path()) {
            files.push(&s.file);
        }
    }
    let mut n = 0;
    for file in files {
        let existed = file.is_file();
        let old = if existed {
            std::fs::read_to_string(file)
                .with_context(|| format!("reading {}", file.display()))?
        } else {
            String::new()
        };
        let mut lines: Vec<String> = old.lines().map(str::to_string).collect();
        let mut dirty = false;
        for s in steps.iter().filter(|s| s.writes() && s.file == file) {
            match &s.op {
                Op::Remove => dirty |= remove_first(&mut lines, &s.line),
                Op::Replace { old } => {
                    match lines.iter().position(|l| l.trim() == old) {
                        Some(i) => lines[i] = s.line.clone(),
                        None => lines.push(s.line.clone()),
                    }
                    dirty = true;
                }
                Op::Add => {
                    lines.push(s.line.clone());
                    dirty = true;
                }
                _ => {}
            }
        }
        if dirty {
            if existed {
                let bak = crate::util::backup(env, file, stamp)?;
                println!("  sauvegarde : {}", env.display(&bak));
            }
            let mut text = lines.join("\n");
            if !lines.is_empty() {
                text.push('\n');
            }
            write_atomic(file, &text)?;
        }
    }
    for s in steps.iter().filter(|s| s.writes()) {
        let key = key_of(&s.line);
        manifest.lines.retain(|e| {
            !(Path::new(&e.file) == s.file && e.module == s.module && key_of(&e.line) == key)
        });
        if matches!(s.op, Op::Add | Op::Replace { .. }) {
            manifest.lines.push(LineEntry {
                file: s.file.to_string_lossy().into_owned(),
                line: s.line.clone(),
                module: s.module.clone(),
                installed_at: time::now_rfc3339(),
            });
        }
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn spec(module: &str, file: &Path, line: &str) -> Spec {
        Spec {
            module: module.into(),
            file: file.into(),
            line: line.into(),
            key: key_of(line),
        }
    }

    fn setup() -> (tempfile::TempDir, PathBuf, Env) {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        fs::create_dir_all(home.join(".config/hypr")).unwrap();
        let f = home.join(".config/hypr/x.conf");
        let env = Env::with_home(&home);
        (d, f, env)
    }

    fn run(env: &Env, specs: &[Spec], m: &mut Manifest) -> Vec<Step> {
        let steps = plan(specs, m, &HashSet::new());
        apply(env, &steps, "t", m).unwrap();
        steps
    }

    #[test]
    fn line_key() {
        assert_eq!(key_of("a:b = 1"), "a:b");
        assert_eq!(key_of("  no equals "), "no equals");
    }

    #[test]
    fn add_is_idempotent_then_removal_touches_nothing_else() {
        let (_d, f, env) = setup();
        fs::write(&f, "a = 1").unwrap(); // without a final line break
        let mut m = Manifest::default();
        let s = [spec("rec", &f, "screencopy:picker = /x")];
        let st = run(&env, &s, &mut m);
        assert_eq!(st[0].op, Op::Add);
        assert_eq!(
            fs::read_to_string(&f).unwrap(),
            "a = 1\nscreencopy:picker = /x\n"
        );
        assert_eq!(m.lines.len(), 1);
        assert_eq!(m.lines[0].module, "rec");
        // Run again: nothing to do.
        let st = run(&env, &s, &mut m);
        assert_eq!(st[0].op, Op::Unchanged);
        assert_eq!(m.lines.len(), 1);
        // The module disappears: its line goes, the other stays.
        let st = run(&env, &[], &mut m);
        assert_eq!(st[0].op, Op::Remove);
        assert_eq!(fs::read_to_string(&f).unwrap(), "a = 1\n");
        assert!(m.lines.is_empty());
    }

    #[test]
    fn foreign_line_never_touched() {
        let (_d, f, env) = setup();
        fs::write(&f, "k = 1\nautre = 2\n").unwrap();
        let mut m = Manifest::default();
        // Identical but not put in place by us: left, not tracked, so never removed.
        let st = run(&env, &[spec("rec", &f, "k = 1")], &mut m);
        assert_eq!(st[0].op, Op::Foreign);
        assert!(m.lines.is_empty());
        run(&env, &[], &mut m);
        assert_eq!(fs::read_to_string(&f).unwrap(), "k = 1\nautre = 2\n");
        // Same key, other value: conflict, nothing written.
        let st = run(&env, &[spec("rec", &f, "k = 9")], &mut m);
        assert!(matches!(st[0].op, Op::Conflict(_)));
        assert_eq!(fs::read_to_string(&f).unwrap(), "k = 1\nautre = 2\n");
        assert_eq!(conflicts(&st), 1);
    }

    #[test]
    fn two_modules_same_key_conflict() {
        let (_d, f, env) = setup();
        let mut m = Manifest::default();
        let st = run(
            &env,
            &[spec("a", &f, "k = 1"), spec("b", &f, "k = 2")],
            &mut m,
        );
        assert!(st.iter().all(|s| matches!(s.op, Op::Conflict(_))));
        assert!(!f.exists());
        assert!(m.lines.is_empty());
    }

    #[test]
    fn changed_value_replaces_the_line_put_in_place() {
        let (_d, f, env) = setup();
        let mut m = Manifest::default();
        run(&env, &[spec("rec", &f, "k = 1")], &mut m);
        let st = run(&env, &[spec("rec", &f, "k = 2")], &mut m);
        assert!(matches!(st[0].op, Op::Replace { .. }));
        assert_eq!(fs::read_to_string(&f).unwrap(), "k = 2\n");
        assert_eq!(m.lines.len(), 1);
        assert_eq!(m.lines[0].line, "k = 2");
    }

    #[test]
    fn line_already_gone_is_forgotten_and_link_refused() {
        let (_d, f, env) = setup();
        let mut m = Manifest::default();
        run(&env, &[spec("rec", &f, "k = 1")], &mut m);
        fs::write(&f, "").unwrap();
        let st = run(&env, &[], &mut m);
        assert_eq!(st[0].op, Op::Forget);
        assert!(m.lines.is_empty());
        // A file that is a link is never modified.
        let cible = f.with_file_name("cible");
        fs::write(&cible, "").unwrap();
        fs::remove_file(&f).unwrap();
        std::os::unix::fs::symlink(&cible, &f).unwrap();
        let st = run(&env, &[spec("rec", &f, "k = 1")], &mut m);
        assert!(matches!(st[0].op, Op::Blocked(_)));
        assert_eq!(fs::read_to_string(&cible).unwrap(), "");
    }

    #[test]
    fn repository_declarations() {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir_all(repo.join("modules/rec")).unwrap();
        fs::create_dir_all(repo.join("config/.poste")).unwrap();
        fs::write(
            repo.join("modules/rec/module.toml"),
            "name = \"rec\"\n[[line]]\nfile = \"~/.config/hypr/x.conf\"\nline = \"k = {home}/p\"\n",
        )
        .unwrap();
        fs::write(
            repo.join("config/.poste/lines.toml"),
            "[[line]]\nfile = \"~/.config/hypr/x.conf\"\nline = \"base = 1\"\n",
        )
        .unwrap();
        let env = Env::with_home(d.path().join("home"));
        let s = desired(&repo, &env).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].module, "rec");
        assert!(s[0].line.starts_with("k = /") && s[0].line.ends_with("home/p"));
        assert_eq!(s[1].module, BASE_OWNER);
        // Outside ~: refused.
        fs::write(
            repo.join("modules/rec/module.toml"),
            "name = \"rec\"\n[[line]]\nfile = \"/etc/x\"\nline = \"a\"\n",
        )
        .unwrap();
        assert!(desired(&repo, &env).is_err());
    }
}
