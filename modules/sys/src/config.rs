//! The user's config: `config/` of the repository mirrors `~` (real names, no prefixes)
//! and is put in place with symbolic links. This module classifies the state of each target, prepares the
//! plan of `poste sys setup` and applies it, never overwriting without a backup.
//!
//! In `config/`:
//! - an ordinary file becomes, in `~`, a link to that file of the repository;
//! - a symbolic link of the repository is recreated as is in `~` (same target);
//! - a folder listed in `config/.poste/link-dirs` is linked as a block;
//! - `config/.poste/modes` gives the restricted permissions (`path mode`), applied again to the
//!   real target of the link; `config/.poste/ignore` lists what `add` refuses.

use crate::manifest::{Entry as ManifestEntry, Manifest};
use crate::util::{backup, copy_tree, replace_with_symlink, sha256_file};
use anyhow::{bail, Context, Result};
use poste_core::{time, Env};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

/// Name of the module in `installed.json`.
pub const MODULE: &str = "config";
/// Metadata folder, at the root of `config/`.
pub const META: &str = ".poste";
const PYCACHE: &str = "__pycache__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Ordinary file: link to the file of the repository.
    File,
    /// Symbolic link of the repository: recreated with the same target.
    Link(PathBuf),
    /// Folder linked as a block (`link-dirs`).
    Dir,
    /// Generated JSON file: content of the repository plus the overrides of the machine (wanted text).
    /// Not a link: an ordinary file written by `setup`.
    Generated(String),
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Path relative to `~`, separated by `/`.
    pub rel: String,
    /// In the repository: `config/<rel>`, where the content is read from.
    pub src: PathBuf,
    /// Target of the link put in `~`: `src`, except in a dry run where the root read is not yet
    /// `current` (the real link will go through `current/config/<rel>`).
    pub link: PathBuf,
    /// Dans `~`.
    pub dest: PathBuf,
    pub kind: Kind,
}

#[derive(Debug)]
pub struct Layout {
    /// `<repository>/config`.
    pub root: PathBuf,
    /// `<data repository>/config`, if declared (`data`) and existing: its files
    /// replace those of the repository with the same path, or are added to them.
    pub data_root: Option<PathBuf>,
    /// Entries that `setup` puts in place (except local files).
    pub entries: Vec<Entry>,
    /// Entries that this machine keeps for itself (`local` of `~/.config/poste/config.toml`):
    /// never linked, never reported as a conflict, never touched.
    pub local: Vec<Entry>,
    /// Paths declared in `local`, normalized (including those the repository does not know).
    pub local_paths: Vec<String>,
    /// JSON overrides of the machine, by relative path (generated files).
    pub overrides: BTreeMap<String, crate::overrides::Sets>,
    pub modes: BTreeMap<String, u32>,
    pub link_dirs: Vec<String>,
}

// ---------------------------------------------------------------- reading the repository

/// Normalizes a path relative to `~`: without `./`, `~/`, or a trailing `/`; refuses `..`.
pub fn normalize_rel(s: &str) -> Result<String> {
    let s = s.trim();
    let s = s.strip_prefix("~/").unwrap_or(s);
    let s = s.strip_prefix("./").unwrap_or(s);
    let s = s.trim_end_matches('/');
    if s.is_empty() || s.starts_with('/') {
        bail!("invalid path: '{s}' (expected: relative to ~)");
    }
    if s.split('/').any(|c| c == ".." || c == "." || c.is_empty()) {
        bail!("invalid path: '{s}'");
    }
    Ok(s.to_string())
}

fn lines_of(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'))
}

pub fn parse_modes(text: &str) -> Result<BTreeMap<String, u32>> {
    let mut out = BTreeMap::new();
    for (n, line) in lines_of(text) {
        let (path, mode) = line
            .rsplit_once(char::is_whitespace)
            .with_context(|| format!("modes, line {n}: expected 'path mode'"))?;
        let m = u32::from_str_radix(mode.trim(), 8)
            .ok()
            .filter(|m| *m <= 0o777)
            .with_context(|| format!("modes, line {n}: invalid octal mode '{mode}'"))?;
        out.insert(normalize_rel(path.trim())?, m);
    }
    Ok(out)
}

pub fn parse_link_dirs(text: &str) -> Result<Vec<String>> {
    lines_of(text).map(|(_, l)| normalize_rel(l)).collect()
}

fn read_meta(root: &Path, name: &str) -> Result<String> {
    let f = root.join(META).join(name);
    match std::fs::read_to_string(&f) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", f.display())),
    }
}

/// Is `rel` one of the local paths, or under one of them?
pub fn is_local(rel: &str, local: &[String]) -> bool {
    local
        .iter()
        .any(|l| rel == l || rel.starts_with(&format!("{l}/")))
}

/// `local` paths of the machine's config, normalized (clear error if one is invalid).
pub fn local_paths(env: &Env) -> Result<Vec<String>> {
    let cfg = poste_core::Config::load(env)?;
    cfg.local
        .iter()
        .map(|l| normalize_rel(l).with_context(|| format!("config.toml, local: '{l}'")))
        .collect()
}

/// Reads `config/`; `None` if the repository has no `config/` folder. The `local` paths of the
/// machine's config are set apart (`Layout::local`).
pub fn load(repo: &Path, env: &Env) -> Result<Option<Layout>> {
    load_with(repo, env, &local_paths(env)?, &override_sets(env)?)
}

/// JSON overrides of the machine's config: (normalized relative path, dotted keys).
pub fn override_sets(env: &Env) -> Result<Vec<(String, crate::overrides::Sets)>> {
    let cfg = poste_core::Config::load(env)?;
    let mut out: Vec<(String, crate::overrides::Sets)> = Vec::new();
    for o in &cfg.overrides {
        let rel = normalize_rel(&o.path)
            .with_context(|| format!("config.toml, override: '{}'", o.path))?;
        let sets = crate::overrides::sets_of(o)?;
        match out.iter_mut().find(|(r, _)| *r == rel) {
            Some((_, s)) => s.extend(sets),
            None => out.push((rel, sets)),
        }
    }
    Ok(out)
}

/// Like `load`, with a given list of local paths (empty: everything is managed).
pub fn load_with(
    repo: &Path,
    env: &Env,
    local: &[String],
    overrides: &[(String, crate::overrides::Sets)],
) -> Result<Option<Layout>> {
    let root = repo.join("config");
    if !root.is_dir() {
        return Ok(None);
    }
    let modes = parse_modes(&read_meta(&root, "modes")?)?;
    let link_dirs = parse_link_dirs(&read_meta(&root, "link-dirs")?)?;
    for d in &link_dirs {
        let md = std::fs::symlink_metadata(root.join(d));
        if !matches!(&md, Ok(m) if m.is_dir()) {
            bail!("link-dirs: '{d}' is not a folder of config/");
        }
        if let Some(o) = link_dirs
            .iter()
            .find(|o| *o != d && d.starts_with(&format!("{o}/")))
        {
            bail!("link-dirs: '{d}' is in '{o}', already linked as a block");
        }
    }
    let mut entries = Vec::new();
    scan("", &link_dirs, env, &root, &mut entries)?;
    let mut modes = modes;
    let data_root = data_config_root(env)?;
    if let Some(droot) = &data_root {
        overlay(droot, env, &link_dirs, &mut entries, &mut modes)?;
    }
    let (local_entries, mut entries): (Vec<Entry>, Vec<Entry>) =
        entries.into_iter().partition(|e| is_local(&e.rel, local));
    let mut applied = BTreeMap::new();
    for (rel, sets) in overrides {
        if is_local(rel, local) {
            continue; // the local file wins: the machine manages it alone
        }
        let Some(e) = entries.iter_mut().find(|e| e.rel == *rel) else {
            bail!("override: '{rel}' is not a file of the repository's config/");
        };
        if e.kind != Kind::File {
            bail!("override: '{rel}' is not an ordinary file of config/");
        }
        let text = std::fs::read_to_string(&e.src)
            .with_context(|| format!("override {rel}: reading {}", e.src.display()))?;
        let gen =
            crate::overrides::generate(&text, sets).with_context(|| format!("override {rel}"))?;
        e.kind = Kind::Generated(gen);
        applied.insert(rel.clone(), sets.clone());
    }
    Ok(Some(Layout {
        root,
        data_root,
        entries,
        local: local_entries,
        local_paths: local.to_vec(),
        overrides: applied,
        modes,
        link_dirs,
    }))
}

/// `config/` of the machine's data repository, if declared and existing.
pub fn data_config_root(env: &Env) -> Result<Option<PathBuf>> {
    let cfg = poste_core::Config::load(env)?;
    Ok(cfg
        .data_path(env)
        .map(|d| d.join("config"))
        .filter(|d| d.is_dir()))
}

/// Puts the files of the data repository on top of those of the repository: same path, it wins;
/// otherwise it is added. No `link-dirs` on the data side, and nothing under a folder linked as a block
/// by the repository (a file cannot be put there without touching the repository).
fn overlay(
    droot: &Path,
    env: &Env,
    link_dirs: &[String],
    entries: &mut Vec<Entry>,
    modes: &mut BTreeMap<String, u32>,
) -> Result<()> {
    if !parse_link_dirs(&read_meta(droot, "link-dirs")?)?.is_empty() {
        bail!(
            "{}: link-dirs is not supported in the data repository",
            droot.join(META).join("link-dirs").display()
        );
    }
    modes.extend(parse_modes(&read_meta(droot, "modes")?)?);
    let mut extra = Vec::new();
    scan("", &[], env, droot, &mut extra)?;
    for e in extra {
        if let Some(d) = link_dirs
            .iter()
            .find(|d| e.rel == **d || e.rel.starts_with(&format!("{d}/")))
        {
            bail!(
                "data repository: '{}' is in '{d}', linked as a block by the repository",
                e.rel
            );
        }
        match entries.iter_mut().find(|x| x.rel == e.rel) {
            Some(x) => *x = e,
            None => entries.push(e),
        }
    }
    entries.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(())
}

impl Layout {
    /// Dry run: the links will put `<link_root>/config/<rel>` (the `current` link, not yet
    /// in place) whereas the content is read from the original root.
    pub fn relink(&mut self, link_root: &Path) {
        for e in self.entries.iter_mut().chain(self.local.iter_mut()) {
            if let Ok(rest) = e.src.strip_prefix(&self.root) {
                e.link = link_root.join("config").join(rest);
            }
        }
    }

    /// Root (`config/` of the repository or of the data repository) this entry comes from.
    pub fn root_of(&self, e: &Entry) -> &Path {
        match &self.data_root {
            Some(d) if e.src.starts_with(d) => d,
            _ => &self.root,
        }
    }
}

fn scan(
    prefix: &str,
    link_dirs: &[String],
    env: &Env,
    dir: &Path,
    out: &mut Vec<Entry>,
) -> Result<()> {
    let mut children: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .flatten()
        .map(|e| e.file_name())
        .collect();
    children.sort();
    for name in children {
        let name_s = name.to_string_lossy().into_owned();
        if prefix.is_empty() && name_s == META {
            continue;
        }
        let rel = if prefix.is_empty() {
            name_s
        } else {
            format!("{prefix}/{name_s}")
        };
        let src = dir.join(&name);
        let md = std::fs::symlink_metadata(&src)?;
        let mk = |kind| Entry {
            rel: rel.clone(),
            src: src.clone(),
            link: src.clone(),
            dest: env.home.join(&rel),
            kind,
        };
        if md.file_type().is_symlink() {
            out.push(mk(Kind::Link(std::fs::read_link(&src)?)));
        } else if md.is_dir() {
            if link_dirs.contains(&rel) {
                out.push(mk(Kind::Dir));
            } else {
                scan(&rel, link_dirs, env, &src, out)?;
            }
        } else if md.is_file() {
            out.push(mk(Kind::File));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- states

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Already linked as planned.
    Linked,
    /// Nothing at the target.
    Missing,
    /// Ordinary file (or folder) identical to the repository.
    Identical,
    /// Ordinary file (or folder) different from the repository.
    Differs,
    /// Link that points elsewhere (`broken`: the target does not exist).
    OtherLink { target: PathBuf, broken: bool },
    /// Link to the same path in another poste root (the clone, an installed
    /// version): the old form, before `current`. Replaced if `installed.json` knows it.
    Moved { target: PathBuf },
    /// Wrong type (folder in place of a file, etc.).
    WrongType(&'static str),
    /// A parent folder is not a folder.
    ParentBlocked(PathBuf),
    /// Generated file up to date (repository plus overrides).
    Generated,
    /// Generated file not modified since our write, but the repository or the overrides
    /// changed: to regenerate.
    Stale,
}

#[derive(Debug, PartialEq, Eq)]
enum Node {
    File(String),
    Link(PathBuf),
}

/// Content of a tree (without `__pycache__`, without empty folders), to compare two folders.
fn tree(dir: &Path) -> Result<BTreeMap<String, Node>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Node>) -> Result<()> {
        for e in std::fs::read_dir(dir)? {
            let e = e?;
            if e.file_name() == PYCACHE {
                continue;
            }
            let p = e.path();
            let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
            let ft = e.file_type()?;
            if ft.is_symlink() {
                out.insert(rel, Node::Link(std::fs::read_link(&p)?));
            } else if ft.is_dir() {
                walk(base, &p, out)?;
            } else {
                out.insert(rel, Node::File(sha256_file(&p)?));
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out)?;
    Ok(out)
}

/// Is `target` the `config/<rel>` of a poste root (the `current` link, an installed
/// version, the clone of the `repo` key)?
pub fn is_poste_target(env: &Env, target: &Path, rel: &str) -> bool {
    let tail = Path::new("config").join(rel);
    if target == env.current_link().join(&tail) {
        return true;
    }
    if let Ok(rest) = target.strip_prefix(env.versions_dir()) {
        let mut c = rest.components();
        if c.next().is_some() && c.as_path() == tail {
            return true;
        }
    }
    poste_core::Config::load(env)
        .ok()
        .and_then(|c| c.repo_path(env))
        .is_some_and(|r| target == r.join(&tail))
}

/// Does `installed.json` know this file of `~` as put in place by `setup`?
fn placed_by_setup(env: &Env, dest: &Path) -> bool {
    Manifest::load(&env.manifest_file())
        .ok()
        .and_then(|m| m.get(dest).map(|x| x.module == MODULE))
        .unwrap_or(false)
}

fn same_target(a: &Path, b: &Path) -> bool {
    a == b
        || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// State of an ordinary file against the wanted generated content.
fn generated_state(e: &Entry, text: &str, env: &Env) -> Result<State> {
    let bytes = std::fs::read(&e.dest)?;
    if bytes == text.as_bytes() {
        return Ok(State::Generated);
    }
    let cur = String::from_utf8_lossy(&bytes);
    // Same JSON, other formatting: we can rewrite without losing anything.
    if let (Ok(a), Ok(b)) = (crate::ojson::parse(&cur), crate::ojson::parse(text)) {
        if a.same(&b) {
            return Ok(State::Identical);
        }
    }
    // Written by us and not touched since: the repository or the overrides changed.
    let ours = Manifest::load(&env.manifest_file())
        .ok()
        .and_then(|m| m.get(&e.dest).map(|x| x.sha256.clone()));
    let mine = format!("{:x}", Sha256::digest(&bytes));
    Ok(if ours.as_deref() == Some(mine.as_str()) {
        State::Stale
    } else {
        State::Differs
    })
}

pub fn state_of(e: &Entry, env: &Env) -> Result<State> {
    let md = match std::fs::symlink_metadata(&e.dest) {
        Ok(m) => m,
        Err(err)
            if err.kind() == std::io::ErrorKind::NotFound
                || err.raw_os_error() == Some(20) /* ENOTDIR */ =>
        {
            // An existing parent must be a folder (or a link to a folder).
            for p in e.dest.ancestors().skip(1) {
                if p == env.home {
                    break;
                }
                match std::fs::symlink_metadata(p) {
                    Ok(m) if m.is_dir() => break,
                    Ok(m) if m.file_type().is_symlink() && p.is_dir() => break,
                    Ok(_) => return Ok(State::ParentBlocked(p.to_path_buf())),
                    Err(_) => continue,
                }
            }
            return Ok(State::Missing);
        }
        Err(err) => return Err(err).with_context(|| format!("reading {}", e.dest.display())),
    };
    if md.file_type().is_symlink() {
        let target = std::fs::read_link(&e.dest)?;
        if target != e.link
            && matches!(e.kind, Kind::File | Kind::Dir | Kind::Generated(_))
            && is_poste_target(env, &target, &e.rel)
        {
            // Link to the very file that `current` designates (the clone in dev), put in place by hand:
            // it is fine as is, setup did not put it and has nothing to take back.
            if !matches!(e.kind, Kind::Generated(_))
                && same_target(&target, &e.src)
                && !placed_by_setup(env, &e.dest)
            {
                return Ok(State::Linked);
            }
            // The old form of a link put by poste: the generated file replaces it like
            // the old link to the repository; a link is put back to `current`.
            return Ok(if matches!(e.kind, Kind::Generated(_)) {
                State::Identical
            } else {
                State::Moved { target }
            });
        }
        let ok = match &e.kind {
            Kind::Link(t) => *t == target,
            Kind::File | Kind::Dir => target == e.link || same_target(&target, &e.src),
            // The old link to the repository: poste had put it, it can be replaced without risk.
            Kind::Generated(_) => {
                if same_target(&target, &e.src) {
                    return Ok(State::Identical);
                }
                false
            }
        };
        return Ok(if ok {
            State::Linked
        } else {
            State::OtherLink {
                broken: !e.dest.exists(),
                target,
            }
        });
    }
    Ok(match (&e.kind, md.is_dir(), md.is_file()) {
        (Kind::Generated(text), false, true) => generated_state(e, text, env)?,
        (Kind::Generated(_), true, _) => State::WrongType("folder in place of a file"),
        (Kind::File, false, true) => {
            if sha256_file(&e.src)? == sha256_file(&e.dest)? {
                State::Identical
            } else {
                State::Differs
            }
        }
        (Kind::Dir, true, _) => {
            if tree(&e.src)? == tree(&e.dest)? {
                State::Identical
            } else {
                State::Differs
            }
        }
        (Kind::File, true, _) => State::WrongType("folder in place of a file"),
        (Kind::Link(_), true, _) => State::WrongType("folder in place of a link"),
        (Kind::Link(_), _, true) => State::WrongType("ordinary file in place of a link"),
        (Kind::Dir, _, true) => State::WrongType("file in place of a folder"),
        _ => State::WrongType("unexpected file type"),
    })
}

// ---------------------------------------------------------------- modes

#[derive(Debug, Clone)]
pub struct ModeFix {
    pub rel: String,
    /// Real path to change (repository for a linked file, `~` for a plain folder).
    pub path: PathBuf,
    pub from: u32,
    pub to: u32,
}

/// Permissions to fix, computed on the real target of the link.
pub fn mode_fixes(layout: &Layout, env: &Env) -> Vec<ModeFix> {
    let mut out = Vec::new();
    for (rel, want) in &layout.modes {
        if is_local(rel, &layout.local_paths) {
            continue;
        }
        let in_repo = match layout.entries.iter().find(|e| e.rel == *rel) {
            Some(e) => e.src.clone(),
            None => layout.root.join(rel),
        };
        let Ok(repo_md) = std::fs::symlink_metadata(&in_repo) else {
            continue;
        };
        if repo_md.file_type().is_symlink() {
            continue;
        }
        let is_entry = layout.entries.iter().any(|e| e.rel == *rel)
            || layout
                .link_dirs
                .iter()
                .any(|d| rel.starts_with(&format!("{d}/")));
        // An ordinary (unlinked) folder: its permissions are those of the folder in `~`.
        let path = if repo_md.is_dir() && !is_entry {
            let home = env.home.join(rel);
            match std::fs::symlink_metadata(&home) {
                Ok(m) if m.is_dir() => home,
                _ => continue,
            }
        } else {
            in_repo
        };
        if let Ok(md) = std::fs::metadata(&path) {
            let from = md.mode() & 0o777;
            if from != *want {
                out.push(ModeFix {
                    rel: rel.clone(),
                    path,
                    from,
                    to: *want,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------- plan

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Repo,
    Home,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Nothing to do (already linked).
    Done,
    /// Target absent: create the link.
    Create,
    /// Identical ordinary: backup then link.
    Replace,
    /// `--take-repo` decision: backup of the target, then link.
    TakeRepo,
    /// `--take-home` decision: the real target is copied into the repository, then link.
    TakeHome,
    /// Conflict, not handled.
    Conflict,
}

#[derive(Debug)]
pub struct Step {
    pub entry: Entry,
    pub state: State,
    pub op: Op,
}

#[derive(Debug, Default)]
pub struct Plan {
    pub steps: Vec<Step>,
    pub modes: Vec<ModeFix>,
    pub warnings: Vec<String>,
}

impl Plan {
    pub fn count(&self, op: &Op) -> usize {
        self.steps.iter().filter(|s| s.op == *op).count()
    }
    pub fn conflicts(&self) -> Vec<&Step> {
        self.steps.iter().filter(|s| s.op == Op::Conflict).collect()
    }
    /// Number of planned writes.
    pub fn writes(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| !matches!(s.op, Op::Done | Op::Conflict))
            .count()
            + self.modes.len()
    }
}

/// Take-back of a generated file: the repository text to write (error if ambiguous).
fn generated_take_home_text(layout: &Layout, e: &Entry) -> Result<String> {
    let sets = layout.overrides.get(&e.rel).cloned().unwrap_or_default();
    let home = std::fs::read_to_string(&e.dest)?;
    let repo = std::fs::read_to_string(&e.src)?;
    crate::overrides::take_home_text(&home, &repo, &sets)
        .with_context(|| format!("--take-home {}", e.rel))
}

fn check_generated_take_home(layout: &Layout, e: &Entry) -> Result<()> {
    generated_take_home_text(layout, e).map(|_| ())
}

/// Checks that a `--take-home` is feasible for this state.
fn check_take_home(e: &Entry, st: &State) -> Result<()> {
    let ok = match (&e.kind, st) {
        (Kind::File, State::Differs) => true,
        (Kind::File, State::OtherLink { broken, .. }) => {
            if *broken {
                bail!(
                    "--take-home {}: the link is broken, nothing to take back",
                    e.rel
                )
            }
            std::fs::metadata(&e.dest)
                .map(|m| m.is_file())
                .unwrap_or(false)
        }
        (Kind::Dir, State::Differs) => true,
        (Kind::Generated(_), State::Differs) => true,
        (Kind::Link(_), State::OtherLink { .. }) => true,
        _ => false,
    };
    if !ok {
        bail!(
            "--take-home {}: impossible in the current state of the target ({}); use --take-repo",
            e.rel,
            describe(st)
        );
    }
    Ok(())
}

pub fn plan(layout: &Layout, env: &Env, decisions: &HashMap<String, Decision>) -> Result<Plan> {
    for k in decisions.keys() {
        if layout.entries.iter().any(|e| e.rel == *k) {
            continue;
        }
        if let Some(d) = layout
            .link_dirs
            .iter()
            .find(|d| k.starts_with(&format!("{d}/")))
        {
            bail!("'{k}' is in the folder linked as a block '{d}': decide on the folder");
        }
        bail!("'{k}' is not an entry of config/");
    }
    let mut p = Plan::default();
    let manifest = Manifest::load(&env.manifest_file())?;
    for e in &layout.entries {
        let state = state_of(e, env)?;
        let decision = decisions.get(&e.rel).copied();
        let op = match (&state, decision) {
            (State::Linked | State::Generated, d) => {
                if d.is_some() {
                    p.warnings
                        .push(format!("{}: already linked, nothing to decide", e.rel));
                }
                Op::Done
            }
            (State::Missing, d) => {
                if d.is_some() {
                    p.warnings
                        .push(format!("{}: target absent, nothing to decide", e.rel));
                }
                Op::Create
            }
            (State::Identical | State::Stale, d) => {
                if d.is_some() {
                    p.warnings.push(format!(
                        "{}: identical to the repository, nothing to decide",
                        e.rel
                    ));
                }
                Op::Replace
            }
            // Put in place by poste (installed.json): put back to the new root, old one backed up.
            (State::Moved { .. }, None)
                if manifest.get(&e.dest).is_some_and(|m| m.module == MODULE) =>
            {
                Op::Replace
            }
            (State::ParentBlocked(_), Some(_)) => bail!(
                "{}: a parent folder is not a folder, impossible to decide",
                e.rel
            ),
            (_, Some(Decision::Repo)) => Op::TakeRepo,
            (st, Some(Decision::Home)) => {
                check_take_home(e, st)?;
                if let Kind::Generated(_) = e.kind {
                    // Clean refusal, before writing anything, if the take-back is ambiguous.
                    check_generated_take_home(layout, e)?;
                }
                Op::TakeHome
            }
            (_, None) => Op::Conflict,
        };
        p.steps.push(Step {
            entry: e.clone(),
            state,
            op,
        });
    }
    p.modes = mode_fixes(layout, env);
    Ok(p)
}

pub fn describe(st: &State) -> String {
    match st {
        State::Linked => "linked".into(),
        State::Missing => "absent".into(),
        State::Identical => "identical ordinary file".into(),
        State::Differs => "different from the repository".into(),
        State::Moved { target } => format!("link to {} (old root)", target.display()),
        State::OtherLink { target, broken } => format!(
            "link to {}{}",
            target.display(),
            if *broken { " (broken)" } else { "" }
        ),
        State::Generated => "generated, up to date".into(),
        State::Stale => "generated, to regenerate".into(),
        State::WrongType(s) => (*s).to_string(),
        State::ParentBlocked(p) => format!("{} is not a folder", p.display()),
    }
}

// ---------------------------------------------------------------- display

pub fn print_plan(env: &Env, plan: &Plan) {
    println!("\nConfig (~ <- config/ by links):");
    let linked = plan.count(&Op::Done);
    for s in &plan.steps {
        let d = env.display(&s.entry.dest);
        let kind = if s.entry.kind == Kind::Dir {
            " (folder)"
        } else {
            ""
        };
        match s.op {
            Op::Done | Op::Conflict => {}
            Op::Create if matches!(s.entry.kind, Kind::Generated(_)) => {
                println!("  generated   {d}  (repository + overrides of the machine)")
            }
            Op::Replace if matches!(s.entry.kind, Kind::Generated(_)) => {
                println!("  regenerated {d}  (repository + overrides of the machine, old one backed up)")
            }
            Op::Create => println!("  linked      {d}{kind}"),
            Op::Replace if matches!(s.state, State::Moved { .. }) => {
                println!("  relinked    {d}{kind}  (link to the old root, put back to current, old one backed up)")
            }
            Op::Replace => {
                println!("  replaced    {d}{kind}  (identical to the repository, old one backed up)")
            }
            Op::TakeRepo => println!(
                "  replaced    {d}{kind}  (--take-repo: version of the repository, old one backed up)"
            ),
            Op::TakeHome => println!(
                "  taken back  {d}{kind}  (--take-home: copied into the repository, old one backed up)"
            ),
        }
    }
    for m in &plan.modes {
        println!("  permissions {}  {:04o} -> {:04o}", m.rel, m.from, m.to);
    }
    for w in &plan.warnings {
        println!("  note        {w}");
    }
    println!(
        "  summary: {linked} already linked, {} to create, {} to replace (identical), {} decision(s) applied, {} permission(s) to fix, {} conflict(s)",
        plan.count(&Op::Create),
        plan.count(&Op::Replace),
        plan.count(&Op::TakeRepo) + plan.count(&Op::TakeHome),
        plan.modes.len(),
        plan.conflicts().len()
    );
}

pub fn print_conflicts(env: &Env, plan: &Plan) {
    let c = plan.conflicts();
    if c.is_empty() {
        return;
    }
    println!(
        "\nConflicts ({}): nothing was touched for these entries.",
        c.len()
    );
    for s in c {
        println!("  {}  [{}]", env.display(&s.entry.dest), describe(&s.state));
        let rel = &s.entry.rel;
        match s.state {
            State::Differs if matches!(s.entry.kind, Kind::Generated(_)) => println!(
                "      generated file (repository + overrides) edited by hand: poste sys diff {rel}\n      regenerate: poste sys setup --take-repo {rel}  or take it back into the repository: --take-home {rel} (puts the overridden values back as in the repository; refused if ambiguous)"
            ),
            State::Differs => println!(
                "      see: poste sys diff {rel}\n      decide: poste sys setup --take-repo {rel}  or  --take-home {rel}"
            ),
            State::ParentBlocked(_) => {
                println!("      to fix by hand: a parent folder is a file")
            }
            _ => println!(
                "      decide: poste sys setup --take-repo {rel}  or  --take-home {rel}"
            ),
        }
    }
}

// ---------------------------------------------------------------- application

fn link_target(e: &Entry) -> PathBuf {
    match &e.kind {
        Kind::Link(t) => t.clone(),
        _ => e.link.clone(),
    }
}

/// Copies what is in `~` (file, link or folder) into the repository.
fn take_home(layout: &Layout, e: &Entry, from: &Path) -> Result<()> {
    match &e.kind {
        Kind::File => {
            let md = std::fs::metadata(from)?;
            let tmp = e
                .src
                .with_file_name(format!(".poste-tmp-{}-takeback", std::process::id()));
            std::fs::copy(from, &tmp)?;
            let exec = md.mode() & 0o111 != 0;
            std::fs::set_permissions(
                &tmp,
                std::fs::Permissions::from_mode(if exec { 0o755 } else { 0o644 }),
            )?;
            std::fs::rename(&tmp, &e.src)?;
            note_restricted_mode(layout, e, md.mode() & 0o777)?;
        }
        Kind::Link(_) => {
            let t = std::fs::read_link(from)?;
            replace_with_symlink(&e.src, &t)?;
        }
        Kind::Dir => merge_tree(from, &e.src)?,
        Kind::Generated(_) => unreachable!("handled by apply_generated"),
    }
    Ok(())
}

/// Copies `src` onto `dst` (without removing anything from `dst`), except `__pycache__`.
fn merge_tree(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_name() == PYCACHE {
            continue;
        }
        let (s, d) = (entry.path(), dst.join(entry.file_name()));
        let ft = entry.file_type()?;
        if ft.is_dir() {
            std::fs::create_dir_all(&d)?;
            merge_tree(&s, &d)?;
        } else if ft.is_symlink() {
            replace_with_symlink(&d, &std::fs::read_link(&s)?)?;
        } else {
            std::fs::create_dir_all(dst)?;
            std::fs::copy(&s, &d)?;
        }
    }
    Ok(())
}

/// A file taken back with restricted permissions: adds its mode to `config/.poste/modes`.
fn note_restricted_mode(layout: &Layout, e: &Entry, mode: u32) -> Result<()> {
    if mode & 0o077 == 0 && !layout.modes.contains_key(&e.rel) {
        upsert_mode_line(layout.root_of(e), &e.rel, mode)?;
        std::fs::set_permissions(&e.src, std::fs::Permissions::from_mode(mode))?;
        println!(
            "  permissions {:04o} noted in config/.poste/modes for {}",
            mode, e.rel
        );
    }
    Ok(())
}

/// Adds or replaces the line of a path in `config/.poste/modes` (keeps the comments).
pub fn upsert_mode_line(root: &Path, rel: &str, mode: u32) -> Result<()> {
    let file = root.join(META).join("modes");
    std::fs::create_dir_all(root.join(META))?;
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let new_line = format!("{rel} {mode:04o}");
    let mut found = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|l| {
            let t = l.trim();
            if !t.starts_with('#') {
                if let Some((p, _)) = t.rsplit_once(char::is_whitespace) {
                    if p.trim() == rel {
                        found = true;
                        return new_line.clone();
                    }
                }
            }
            l.to_string()
        })
        .collect();
    if !found {
        lines.push(new_line);
    }
    std::fs::write(&file, lines.join("\n") + "\n")?;
    Ok(())
}

/// Fingerprint to note in `installed.json`: content of a file, otherwise the target of the link.
fn record_hash(e: &Entry) -> Result<String> {
    Ok(match &e.kind {
        Kind::File => sha256_file(&e.src)?,
        Kind::Generated(text) => format!("{:x}", Sha256::digest(text.as_bytes())),
        _ => format!(
            "{:x}",
            Sha256::digest(link_target(e).to_string_lossy().as_bytes())
        ),
    })
}

/// Notes the entry in the manifest; `true` if something changed.
pub fn record(manifest: &mut Manifest, e: &Entry) -> Result<bool> {
    let path = e.dest.to_string_lossy().into_owned();
    let sha = record_hash(e)?;
    if manifest
        .get(&e.dest)
        .is_some_and(|m| m.sha256 == sha && m.module == MODULE)
    {
        return Ok(false);
    }
    manifest.upsert(ManifestEntry {
        path,
        sha256: sha,
        module: MODULE.into(),
        installed_at: time::now_rfc3339(),
    });
    Ok(true)
}

/// Applies the plan. Returns the number of entries written. Updates `manifest` (to be
/// saved by the caller, who also knows how to save the entries that are already linked).
pub fn apply(
    layout: &Layout,
    env: &Env,
    plan: &Plan,
    stamp: &str,
    manifest: &mut Manifest,
    manifest_file: &Path,
) -> Result<usize> {
    let mut n = 0;
    for s in &plan.steps {
        let e = &s.entry;
        match s.op {
            Op::Done | Op::Conflict => continue,
            Op::Create | Op::Replace | Op::TakeRepo | Op::TakeHome
                if matches!(e.kind, Kind::Generated(_)) =>
            {
                let text = apply_generated(layout, env, e, &s.op, stamp)?;
                record_text(manifest, e, &text);
                manifest.save(manifest_file)?;
                n += 1;
                continue;
            }
            Op::Create => {
                if let Some(parent) = e.dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                symlink(link_target(e), &e.dest)
                    .with_context(|| format!("link {}", e.dest.display()))?;
            }
            Op::Replace | Op::TakeRepo | Op::TakeHome => {
                if s.op == Op::TakeHome {
                    // A folder is moved by the backup: we copy it back from there.
                    // For the rest, we copy first, then back up the original.
                    let bak = if e.kind == Kind::Dir {
                        let bak = backup(env, &e.dest, stamp)?;
                        take_home(layout, e, &bak)?;
                        bak
                    } else {
                        take_home(layout, e, &e.dest)?;
                        backup(env, &e.dest, stamp)?
                    };
                    println!("  backup: {}", env.display(&bak));
                    if matches!(e.kind, Kind::Link(_)) && state_of(e, env)? == State::Linked {
                        record(manifest, e)?;
                        manifest.save(manifest_file)?;
                        n += 1;
                        continue;
                    }
                } else {
                    let bak = backup(env, &e.dest, stamp)?;
                    println!("  backup: {}", env.display(&bak));
                }
                place_link(e)?;
            }
        }
        record(manifest, e)?;
        manifest.save(manifest_file)?;
        n += 1;
    }
    for m in &plan.modes {
        std::fs::set_permissions(&m.path, std::fs::Permissions::from_mode(m.to))
            .with_context(|| format!("cannot set permissions of {}", m.path.display()))?;
        println!("  permissions {:04o}: {}", m.to, m.rel);
        n += 1;
    }
    Ok(n)
}

/// Writes an ordinary file atomically (a possible link is replaced, not followed).
fn write_text(dest: &Path, text: &str) -> Result<()> {
    let parent = dest
        .parent()
        .context("destination without a parent folder")?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".poste-tmp-{}-gen", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, dest).with_context(|| format!("writing {}", dest.display()))
}

/// Puts a generated file in place (backup of the old one, possible take-back into the repository). Returns
/// the text written.
fn apply_generated(layout: &Layout, env: &Env, e: &Entry, op: &Op, stamp: &str) -> Result<String> {
    let Kind::Generated(text) = &e.kind else {
        unreachable!("generated file");
    };
    let mut text = text.clone();
    if *op == Op::TakeHome {
        // The repository receives the file of ~ without the overridden values; the generated file derives from it.
        let new_repo = generated_take_home_text(layout, e)?;
        let sets = layout.overrides.get(&e.rel).cloned().unwrap_or_default();
        text = crate::overrides::generate(&new_repo, &sets)?;
        write_text(&e.src, &new_repo)?;
        println!(
            "  taken back into the repository (overridden values put back): config/{}",
            e.rel
        );
    }
    if std::fs::symlink_metadata(&e.dest).is_ok() {
        let bak = backup(env, &e.dest, stamp)?;
        println!("  backup: {}", env.display(&bak));
    }
    write_text(&e.dest, &text)?;
    Ok(text)
}

/// Notes a generated file in the manifest (fingerprint of the written text).
fn record_text(manifest: &mut Manifest, e: &Entry, text: &str) {
    manifest.upsert(ManifestEntry {
        path: e.dest.to_string_lossy().into_owned(),
        sha256: format!("{:x}", Sha256::digest(text.as_bytes())),
        module: MODULE.into(),
        installed_at: time::now_rfc3339(),
    });
}

/// Puts the final link in place (the target was backed up; a folder was already moved).
fn place_link(e: &Entry) -> Result<()> {
    let target = link_target(e);
    match std::fs::symlink_metadata(&e.dest) {
        Ok(m) if m.is_dir() => bail!("{} is still a folder", e.dest.display()),
        Ok(_) => replace_with_symlink(&e.dest, &target),
        Err(_) => {
            if let Some(parent) = e.dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            symlink(&target, &e.dest).with_context(|| format!("link {}", e.dest.display()))
        }
    }
}

/// Records in the manifest all the entries that are already linked (idempotent); `true` if changed.
pub fn record_linked(plan: &Plan, manifest: &mut Manifest) -> Result<bool> {
    let mut changed = false;
    for s in &plan.steps {
        if s.op == Op::Done {
            changed |= record(manifest, &s.entry)?;
        }
    }
    Ok(changed)
}

// ---------------------------------------------------------------- paths given by the user

/// Path relative to `~` for an argument: `~/x`, absolute under `~`, relative to the current
/// folder if it exists under `~`, otherwise relative to `~`. Does not follow links.
pub fn rel_of(env: &Env, arg: &str) -> Result<String> {
    let p = if arg == "~" || arg.starts_with("~/") {
        env.expand(arg)
    } else if arg.starts_with('/') {
        PathBuf::from(arg)
    } else {
        let cwd = std::env::current_dir().ok().map(|c| c.join(arg));
        match cwd {
            Some(c) if c.starts_with(&env.home) && std::fs::symlink_metadata(&c).is_ok() => c,
            _ => env.home.join(arg),
        }
    };
    let mut parts: Vec<String> = Vec::new();
    for c in p
        .strip_prefix(&env.home)
        .map_err(|_| anyhow::anyhow!("{} is not under the home folder", p.display()))?
        .components()
    {
        match c {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => bail!("invalid path: {arg}"),
        }
    }
    if parts.is_empty() {
        bail!("invalid path: {arg}");
    }
    Ok(parts.join("/"))
}

fn entry_for<'a>(layout: &'a Layout, rel: &str) -> Result<&'a Entry> {
    if let Some(e) = layout
        .entries
        .iter()
        .chain(layout.local.iter())
        .find(|e| e.rel == rel)
    {
        return Ok(e);
    }
    if let Some(d) = layout
        .link_dirs
        .iter()
        .find(|d| rel.starts_with(&format!("{d}/")))
    {
        bail!("{rel} is in the folder linked as a block '{d}': use that folder");
    }
    bail!("{rel} is not managed by config/")
}

// ---------------------------------------------------------------- diff

/// `poste sys diff [<path>]`: returns `true` if there are differences.
pub fn diff(layout: &Layout, env: &Env, arg: Option<&str>) -> Result<bool> {
    let entries: Vec<&Entry> = match arg {
        Some(a) => vec![entry_for(layout, &rel_of(env, a)?)?],
        // Without argument: only what `setup` manages (local files are meant to differ).
        None => layout.entries.iter().collect(),
    };
    let mut any = false;
    let mut others: Vec<(String, String)> = Vec::new();
    for e in entries {
        let st = state_of(e, env)?;
        if arg.is_some() && is_local(&e.rel, &layout.local_paths) {
            println!(
                "(file local to this machine: setup does not touch it, the difference is normal)"
            );
        }
        match st {
            State::Linked | State::Identical | State::Generated => {
                if arg.is_some() {
                    println!("{}: {}, no difference.", e.rel, describe(&st));
                }
            }
            State::Missing => {
                if arg.is_some() {
                    println!("{}: absent from ~ (setup will create the link).", e.rel);
                }
            }
            State::Differs | State::Stale => {
                any = true;
                println!("=== {} (repository: -, ~: +)", e.rel);
                // Generated file: we compare with what setup would write (repository + overrides).
                let gen_tmp = if let Kind::Generated(text) = &e.kind {
                    let t =
                        std::env::temp_dir().join(format!(".poste-diff-{}", std::process::id()));
                    std::fs::write(&t, text)?;
                    Some(t)
                } else {
                    None
                };
                let _ = std::io::Write::flush(&mut std::io::stdout());
                let mut cmd = std::process::Command::new("diff");
                cmd.args(["-ru", "-x", PYCACHE, "--label", &format!("repo/{}", e.rel)]);
                cmd.arg("--label").arg(format!("home/{}", e.rel));
                cmd.arg(gen_tmp.as_deref().unwrap_or(&e.src)).arg(&e.dest);
                let status = cmd.status().context("cannot run diff")?;
                if let Some(t) = &gen_tmp {
                    let _ = std::fs::remove_file(t);
                }
                if status.code().is_some_and(|c| c > 1) {
                    bail!("diff failed for {}", e.rel);
                }
            }
            other => {
                any = true;
                if arg.is_some() {
                    println!("{}: {}", e.rel, describe(&other));
                    if let (Kind::Link(t), State::OtherLink { target, .. }) = (&e.kind, &other) {
                        println!(
                            "  repository: link to {}\n  ~         : link to {}",
                            t.display(),
                            target.display()
                        );
                    }
                } else {
                    others.push((e.rel.clone(), describe(&other)));
                }
            }
        }
    }
    if !others.is_empty() {
        println!("\nOther conflicts (no diff possible):");
        for (p, d) in others {
            println!("  {p}  [{d}]");
        }
    }
    if !any && arg.is_none() {
        println!("No difference: everything in place is linked or identical.");
    }
    Ok(any)
}

// ---------------------------------------------------------------- add / forget

/// Permissions to note for an added file: restricted only if there are no group/other permissions.
fn restricted(mode: u32) -> Option<u32> {
    (mode & 0o077 == 0).then_some(mode & 0o777)
}

/// Paths (files and links) under `p` for `add`, except `__pycache__`.
fn collect_files(p: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let md = std::fs::symlink_metadata(p)?;
    if md.is_dir() && !md.file_type().is_symlink() {
        let mut kids: Vec<_> = std::fs::read_dir(p)?.flatten().map(|e| e.path()).collect();
        kids.sort();
        for k in kids {
            if k.file_name().is_some_and(|n| n == PYCACHE) {
                continue;
            }
            collect_files(&k, out)?;
        }
    } else {
        out.push(p.to_path_buf());
    }
    Ok(())
}

pub fn add(env: &Env, repo: &Path, arg: &str, dry_run: bool, manifest_file: &Path) -> Result<()> {
    let root = repo.join("config");
    let rel = rel_of(env, arg)?;
    let ignore = crate::ignore::Ignore::load(&root.join(META).join("ignore"))?;
    if let Some(rule) = ignore.matching(&rel) {
        bail!(
            "refused: {rel} matches the rule '{rule}' (config/.poste/ignore or built-in rules: secrets, caches, regenerated files)"
        );
    }
    let home_path = env.home.join(&rel);
    if std::fs::symlink_metadata(&home_path).is_err() {
        bail!("{} does not exist", env.display(&home_path));
    }
    let mut files = Vec::new();
    collect_files(&home_path, &mut files)?;
    let mut manifest = Manifest::load(manifest_file)?;
    let stamp = time::now_compact();
    let (mut added, mut skipped) = (0, 0);
    for f in files {
        let frel = f.strip_prefix(&env.home)?.to_string_lossy().into_owned();
        if let Some(rule) = ignore.matching(&frel) {
            println!("  ignored: {frel} (rule '{rule}')");
            skipped += 1;
            continue;
        }
        let dest_repo = root.join(&frel);
        let md = std::fs::symlink_metadata(&f)?;
        // Already in the repository (link to config/, or under a linked folder)?
        if let (Ok(c), Ok(r)) = (std::fs::canonicalize(&f), std::fs::canonicalize(&root)) {
            if c.starts_with(&r) {
                println!("  already managed: {frel}");
                skipped += 1;
                continue;
            }
        }
        if std::fs::symlink_metadata(&dest_repo).is_ok() {
            println!(
                "  already exists in the repository: {frel} (decide with poste sys setup --take-home/--take-repo)"
            );
            skipped += 1;
            continue;
        }
        if dry_run {
            println!("  would add: {frel}");
            added += 1;
            continue;
        }
        if let Some(parent) = dest_repo.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if md.file_type().is_symlink() {
            symlink(std::fs::read_link(&f)?, &dest_repo)?;
            println!("  added (link): {frel}");
        } else {
            let bak = backup(env, &f, &stamp)?;
            std::fs::copy(&f, &dest_repo)?;
            if sha256_file(&f)? != sha256_file(&dest_repo)? {
                bail!("copy of {frel} differs from the original, giving up (original intact)");
            }
            let exec = md.mode() & 0o111 != 0;
            std::fs::set_permissions(
                &dest_repo,
                std::fs::Permissions::from_mode(if exec { 0o755 } else { 0o644 }),
            )?;
            if let Some(m) = restricted(md.mode()) {
                upsert_mode_line(&root, &frel, m)?;
                std::fs::set_permissions(&dest_repo, std::fs::Permissions::from_mode(m))?;
            }
            replace_with_symlink(&f, &dest_repo)?;
            println!("  added: {frel} (backup: {})", env.display(&bak));
        }
        let e = Entry {
            rel: frel.clone(),
            link: dest_repo.clone(),
            src: dest_repo,
            dest: f.clone(),
            kind: if md.file_type().is_symlink() {
                Kind::Link(std::fs::read_link(&f)?)
            } else {
                Kind::File
            },
        };
        record(&mut manifest, &e)?;
        added += 1;
    }
    if !dry_run {
        manifest.save(manifest_file)?;
    }
    println!(
        "{added} added{}; to check then commit in the repository.",
        if skipped > 0 {
            format!(", {skipped} ignored")
        } else {
            String::new()
        }
    );
    if dry_run {
        println!("Dry run (--dry-run): nothing was written.");
    }
    Ok(())
}

pub fn forget(layout: &Layout, env: &Env, arg: &str, manifest_file: &Path) -> Result<()> {
    let rel = rel_of(env, arg)?;
    let e = entry_for(layout, &rel)?;
    let st = state_of(e, env)?;
    if st != State::Linked {
        bail!("{rel} is not linked ({}): nothing to remove", describe(&st));
    }
    match &e.kind {
        Kind::File => {
            let tmp = e
                .dest
                .with_file_name(format!(".poste-tmp-{}-forget", std::process::id()));
            std::fs::copy(&e.src, &tmp)?;
            std::fs::rename(&tmp, &e.dest)?;
        }
        Kind::Dir => {
            let tmp = e
                .dest
                .with_file_name(format!(".poste-tmp-{}-forget", std::process::id()));
            copy_tree(&e.src, &tmp)?;
            std::fs::remove_file(&e.dest)?;
            std::fs::rename(&tmp, &e.dest)?;
        }
        Kind::Link(_) | Kind::Generated(_) => {}
    }
    let mut manifest = Manifest::load(manifest_file)?;
    let p = e.dest.to_string_lossy().into_owned();
    if manifest.files.iter().any(|m| m.path == p) {
        manifest.files.retain(|m| m.path != p);
        manifest.save(manifest_file)?;
    }
    println!(
        "{}: the link is replaced by an ordinary file (copy of the repository).",
        env.display(&e.dest)
    );
    println!(
        "The file stays in the repository until your commit: remove it with 'git rm -r config/{rel}' (and its line in config/.poste/modes or link-dirs if needed), otherwise the next setup would link it again."
    );
    Ok(())
}

// ---------------------------------------------------------------- status

/// Files that this machine keeps for itself (`local`), with their state against the repository.
pub fn print_local(layout: &Layout, env: &Env) -> Result<()> {
    if layout.local_paths.is_empty() {
        return Ok(());
    }
    println!(
        "\nLocal files ({}; config.toml, key local: poste neither links nor touches them)",
        layout.local.len()
    );
    for e in &layout.local {
        let st = state_of(e, env)?;
        let d = match &st {
            State::Linked => {
                "still linked to the repository (poste sys forget to make it an ordinary file)"
                    .to_string()
            }
            State::Missing => "absent from ~".to_string(),
            State::Identical => "identical to the repository".to_string(),
            State::Differs => "different from the repository (poste sys diff to see)".to_string(),
            other => describe(other),
        };
        println!("  local        {}  [{d}]", env.display(&e.dest));
    }
    for l in &layout.local_paths {
        if !layout
            .local
            .iter()
            .any(|e| is_local(&e.rel, std::slice::from_ref(l)))
        {
            println!("  unknown      ~/{l}  (no config/ entry matches)");
        }
    }
    Ok(())
}

pub fn print_status(layout: &Layout, env: &Env, manifest: &Manifest) -> Result<()> {
    println!("\nConfig ({} entries in config/)", layout.entries.len());
    let plan = plan(layout, env, &HashMap::new())?;
    let mut linked = 0;
    let mut lines: Vec<String> = Vec::new();
    let (mut missing, mut broken, mut replaced, mut conflicts) = (0, 0, 0, 0);
    let (mut generated, mut stale) = (0, 0);
    let tracked = |e: &Entry| manifest.get(&e.dest).is_some();
    for s in &plan.steps {
        let d = env.display(&s.entry.dest);
        let rel = &s.entry.rel;
        match &s.state {
            State::Linked => linked += 1,
            State::Generated => generated += 1,
            State::Stale => {
                stale += 1;
                lines.push(format!(
                    "  to regenerate {d}  (the repository or the overrides changed: poste sys setup)"
                ));
            }
            State::Differs if matches!(s.entry.kind, Kind::Generated(_)) => {
                conflicts += 1;
                lines.push(format!(
                    "  generated, edited by hand  {d}  (poste sys diff {rel}; poste sys setup --take-repo {rel} to regenerate, --take-home {rel} to take it back into the repository)"
                ));
            }
            State::Missing => {
                missing += 1;
                lines.push(format!(
                    "  missing      {d}  (poste sys setup creates the link)"
                ));
            }
            State::OtherLink {
                broken: true,
                target,
            } => {
                broken += 1;
                lines.push(format!(
                    "  broken link  {d}  -> {}  (poste sys setup --take-repo {rel})",
                    target.display()
                ));
            }
            State::Moved { target } if tracked(&s.entry) => {
                missing += 1;
                lines.push(format!(
                    "  to relink    {d}  -> {}  (old root: poste sys setup puts it back to current)",
                    target.display()
                ));
            }
            State::Identical if tracked(&s.entry) => {
                replaced += 1;
                lines.push(format!(
                    "  replaced by an ordinary file (identical to the repository)  {d}  (poste sys setup links it)"
                ));
            }
            State::Differs if tracked(&s.entry) => {
                replaced += 1;
                lines.push(format!(
                    "  replaced by an ordinary file, modified  {d}  (an app rewrote the file: poste sys setup --take-home {rel} to take it back, or --take-repo {rel})"
                ));
            }
            State::Identical => {
                missing += 1;
                lines.push(format!(
                    "  not linked yet  {d}  (identical file, poste sys setup links it)"
                ));
            }
            other => {
                conflicts += 1;
                lines.push(format!(
                    "  conflict     {d}  [{}]  (poste sys diff {rel})",
                    describe(other)
                ));
            }
        }
    }
    println!("  linked            {linked}");
    if !layout.overrides.is_empty() {
        println!(
            "  generated         {generated}  (repository + overrides; {stale} to regenerate)"
        );
    }
    println!("  missing           {missing}");
    println!("  broken links      {broken}");
    println!("  replaced          {replaced}  (link turned into an ordinary file)");
    println!("  conflicts         {conflicts}");
    println!("  permissions to fix {}", plan.modes.len());
    for l in lines {
        println!("{l}");
    }
    for m in &plan.modes {
        println!("  permissions  {}  {:04o} -> {:04o}", m.rel, m.from, m.to);
    }
    print_local(layout, env)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Temporary repository and HOME: never the real `~`.
    struct Fx {
        _d: tempfile::TempDir,
        repo: PathBuf,
        env: Env,
    }

    fn fx() -> Fx {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        let home = d.path().join("home");
        fs::create_dir_all(repo.join("config")).unwrap();
        fs::create_dir_all(&home).unwrap();
        Fx {
            repo,
            env: Env::with_home(home),
            _d: d,
        }
    }

    impl Fx {
        fn cfg(&self, rel: &str, content: &str) -> PathBuf {
            let p = self.repo.join("config").join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, content).unwrap();
            p
        }
        fn home(&self, rel: &str) -> PathBuf {
            self.env.home.join(rel)
        }
        fn put(&self, rel: &str, content: &str) -> PathBuf {
            let p = self.home(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, content).unwrap();
            p
        }
        fn meta(&self, name: &str, text: &str) {
            let d = self.repo.join("config").join(META);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join(name), text).unwrap();
        }
        fn layout(&self) -> Layout {
            load(&self.repo, &self.env).unwrap().unwrap()
        }
        fn plan(&self, dec: &[(&str, Decision)]) -> Plan {
            let m: HashMap<String, Decision> =
                dec.iter().map(|(k, v)| (k.to_string(), *v)).collect();
            plan(&self.layout(), &self.env, &m).unwrap()
        }
        fn apply(&self, p: &Plan) -> Manifest {
            let mf = self.env.manifest_file();
            let mut m = Manifest::load(&mf).unwrap();
            apply(&self.layout(), &self.env, p, "20260101-000000", &mut m, &mf).unwrap();
            record_linked(p, &mut m).unwrap();
            m.save(&mf).unwrap();
            m
        }
        fn state(&self, rel: &str) -> State {
            let l = self.layout();
            state_of(l.entries.iter().find(|e| e.rel == rel).unwrap(), &self.env).unwrap()
        }
        fn backups(&self) -> Vec<String> {
            fs::read_dir(self.env.backup_dir())
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default()
        }
    }

    #[test]
    fn link_to_the_clone_put_by_hand_in_dev_is_linked_unless_setup_put_it() {
        let f = fx();
        let src = f.cfg(".config/a/x", "1");
        f.cfg(".config/a/y", "2");
        // Dev mode: `current` designates the clone (key `repo`).
        fs::create_dir_all(f.env.config_file().parent().unwrap()).unwrap();
        fs::write(
            f.env.config_file(),
            format!("repo = \"{}\"\n", f.repo.display()),
        )
        .unwrap();
        fs::create_dir_all(f.env.current_link().parent().unwrap()).unwrap();
        symlink(&f.repo, f.env.current_link()).unwrap();
        let state = |rel: &str| {
            let l = load(&f.env.current_link(), &f.env).unwrap().unwrap();
            state_of(l.entries.iter().find(|e| e.rel == rel).unwrap(), &f.env).unwrap()
        };
        for rel in [".config/a/x", ".config/a/y"] {
            let d = f.home(rel);
            fs::create_dir_all(d.parent().unwrap()).unwrap();
            symlink(f.repo.join("config").join(rel), &d).unwrap();
        }
        // Put in place by hand, unknown to installed.json: already right, neither put back nor conflict.
        assert_eq!(state(".config/a/x"), State::Linked);
        // Put in place by setup (noted): put back to current, as in the migration.
        let mut m = Manifest::default();
        m.upsert(ManifestEntry {
            path: f.home(".config/a/y").to_string_lossy().into_owned(),
            sha256: "x".into(),
            module: MODULE.into(),
            installed_at: "t".into(),
        });
        m.save(&f.env.manifest_file()).unwrap();
        assert!(matches!(state(".config/a/y"), State::Moved { .. }));
        // A link to another version than the one of current stays a conflict.
        let other = f.repo.parent().unwrap().join("other");
        fs::create_dir_all(other.join("config/.config/a")).unwrap();
        fs::write(other.join("config/.config/a/x"), "1").unwrap();
        fs::remove_file(f.home(".config/a/x")).unwrap();
        symlink(other.join("config/.config/a/x"), f.home(".config/a/x")).unwrap();
        assert!(matches!(state(".config/a/x"), State::OtherLink { .. }));
        assert!(src.is_file());
    }

    #[test]
    fn parsing_of_metadata_files() {
        let m = parse_modes("# c\n.config/foot/foot.ini 0600\n.a b/c 0700\n").unwrap();
        assert_eq!(m[".config/foot/foot.ini"], 0o600);
        assert_eq!(m[".a b/c"], 0o700);
        assert!(parse_modes("x 0999").is_err());
        assert!(parse_modes("x").is_err());
        assert!(parse_modes("/abs 0600").is_err());
        assert_eq!(parse_link_dirs("# c\n.a/b/\n").unwrap(), vec![".a/b"]);
        assert!(parse_link_dirs("../x").is_err());
    }

    #[test]
    fn reading_the_repository_links_and_meta() {
        let f = fx();
        f.cfg(".config/hypr/bindings.lua", "x");
        f.cfg(".bashrc", "y");
        symlink("../../.agents/s", {
            let p = f.repo.join("config/.claude/skills");
            fs::create_dir_all(&p).unwrap();
            p.join("kris")
        })
        .unwrap();
        f.meta("modes", ".bashrc 0600\n");
        let l = f.layout();
        let rels: Vec<&str> = l.entries.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(
            rels,
            vec![
                ".bashrc",
                ".claude/skills/kris",
                ".config/hypr/bindings.lua"
            ]
        );
        assert_eq!(
            l.entries[1].kind,
            Kind::Link(PathBuf::from("../../.agents/s"))
        );
        assert!(load(&fx().repo.join("nope"), &f.env).unwrap().is_none());
    }

    #[test]
    fn data_repository_on_top_of_the_repository() {
        let f = fx();
        f.cfg(".config/a", "repo a");
        f.cfg(".config/b", "example");
        f.cfg(".config/bloc/x", "x");
        f.meta("link-dirs", ".config/bloc\n");
        let data = f.repo.parent().unwrap().join("data");
        let dcfg = |rel: &str, s: &str| {
            let p = data.join("config").join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, s).unwrap();
        };
        dcfg(".config/b", "real b");
        dcfg(".config/c", "machine own");
        dcfg(".poste/modes", ".config/c 0600\n");
        fs::create_dir_all(f.env.config_file().parent().unwrap()).unwrap();
        fs::write(
            f.env.config_file(),
            format!("data = \"{}\"\n", data.display()),
        )
        .unwrap();

        let l = f.layout();
        assert_eq!(l.data_root.as_deref(), Some(data.join("config").as_path()));
        let src = |rel: &str| l.entries.iter().find(|e| e.rel == rel).unwrap().src.clone();
        assert_eq!(src(".config/a"), f.repo.join("config/.config/a"));
        assert_eq!(src(".config/b"), data.join("config/.config/b"));
        assert_eq!(src(".config/c"), data.join("config/.config/c"));
        assert_eq!(l.modes[".config/c"], 0o600);
        let e = l.entries.iter().find(|e| e.rel == ".config/c").unwrap();
        assert_eq!(l.root_of(e), data.join("config"));

        let p = f.plan(&[]);
        f.apply(&p);
        assert_eq!(fs::read_to_string(f.home(".config/b")).unwrap(), "real b");
        assert_eq!(f.state(".config/c"), State::Linked);
        assert_eq!(
            fs::metadata(data.join("config/.config/c")).unwrap().mode() & 0o777,
            0o600
        );

        // Nothing under a folder linked as a block by the repository.
        dcfg(".config/bloc/y", "y");
        assert!(load(&f.repo, &f.env).is_err());
    }

    #[test]
    fn every_target_state() {
        let f = fx();
        f.cfg("linked", "a");
        f.cfg("absent", "a");
        f.cfg("identical", "a");
        f.cfg("different", "a");
        f.cfg("elsewhere", "a");
        f.cfg("broken", "a");
        f.cfg("folder", "a");
        f.cfg("under/file", "a");
        symlink(f.repo.join("config/linked"), f.home("linked")).unwrap();
        f.put("identical", "a");
        f.put("different", "b");
        f.put("other", "a");
        symlink(f.home("other"), f.home("elsewhere")).unwrap();
        symlink(f.home("does-not-exist"), f.home("broken")).unwrap();
        fs::create_dir(f.home("folder")).unwrap();
        f.put("under", "i am a file");
        assert_eq!(f.state("linked"), State::Linked);
        assert_eq!(f.state("absent"), State::Missing);
        assert_eq!(f.state("identical"), State::Identical);
        assert_eq!(f.state("different"), State::Differs);
        assert!(matches!(
            f.state("elsewhere"),
            State::OtherLink { broken: false, .. }
        ));
        assert!(matches!(
            f.state("broken"),
            State::OtherLink { broken: true, .. }
        ));
        assert!(matches!(f.state("folder"), State::WrongType(_)));
        assert!(matches!(f.state("under/file"), State::ParentBlocked(_)));
    }

    #[test]
    fn application_then_rerun_up_to_date() {
        let f = fx();
        let src = f.cfg(".config/a/x.conf", "content");
        f.cfg(".config/a/y.conf", "other");
        f.put(".config/a/y.conf", "other");
        let p = f.plan(&[]);
        assert_eq!(p.count(&Op::Create), 1);
        assert_eq!(p.count(&Op::Replace), 1);
        assert_eq!(p.writes(), 2);
        // The preview writes nothing.
        assert!(fs::symlink_metadata(f.home(".config/a/x.conf")).is_err());
        assert!(f.backups().is_empty());
        let m = f.apply(&p);
        assert_eq!(fs::read_link(f.home(".config/a/x.conf")).unwrap(), src);
        assert!(f.home(".config/a/y.conf").is_symlink());
        // Editing the linked file edits the repository.
        fs::write(f.home(".config/a/x.conf"), "changed").unwrap();
        assert_eq!(fs::read_to_string(&src).unwrap(), "changed");
        // The old identical file was backed up.
        let b = f.backups();
        assert_eq!(b.len(), 1, "{b:?}");
        assert!(b[0].starts_with(".config_a_y.conf.20260101-000000"));
        assert_eq!(
            fs::read_to_string(f.env.backup_dir().join(&b[0])).unwrap(),
            "other"
        );
        // installed.json holds the two entries of the config module.
        assert_eq!(m.files.iter().filter(|e| e.module == MODULE).count(), 2);
        // Run again: nothing to do.
        let p2 = f.plan(&[]);
        assert_eq!(p2.writes(), 0);
        assert_eq!(p2.count(&Op::Done), 2);
        assert!(p2.conflicts().is_empty());
    }

    #[test]
    fn conflicts_listed_without_blocking_the_rest() {
        let f = fx();
        f.cfg("to-create", "1");
        f.cfg("conflict", "repo");
        f.put("conflict", "home");
        let p = f.plan(&[]);
        assert_eq!(p.conflicts().len(), 1);
        assert_eq!(p.count(&Op::Create), 1);
        f.apply(&p);
        assert!(f.home("to-create").is_symlink());
        // The file in conflict is intact, nothing was backed up or overwritten.
        assert_eq!(fs::read_to_string(f.home("conflict")).unwrap(), "home");
        assert!(!f.home("conflict").is_symlink());
        assert!(f.backups().is_empty());
        assert_eq!(f.plan(&[]).conflicts().len(), 1);
    }

    #[test]
    fn decision_take_repo() {
        let f = fx();
        let src = f.cfg("c", "repo");
        f.put("c", "home");
        f.apply(&f.plan(&[("c", Decision::Repo)]));
        assert_eq!(fs::read_link(f.home("c")).unwrap(), src);
        assert_eq!(fs::read_to_string(&src).unwrap(), "repo");
        let b = f.backups();
        assert_eq!(b.len(), 1);
        assert_eq!(
            fs::read_to_string(f.env.backup_dir().join(&b[0])).unwrap(),
            "home"
        );
    }

    #[test]
    fn decision_take_home_copies_into_the_repository() {
        let f = fx();
        let src = f.cfg("c", "repo");
        let h = f.put("secret.conf", "x");
        f.cfg("secret.conf", "old");
        f.put("c", "home");
        fs::set_permissions(f.home("c"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&h, fs::Permissions::from_mode(0o600)).unwrap();
        f.apply(&f.plan(&[("c", Decision::Home), ("secret.conf", Decision::Home)]));
        assert_eq!(fs::read_to_string(&src).unwrap(), "home");
        assert_eq!(fs::metadata(&src).unwrap().mode() & 0o777, 0o755);
        assert!(f.home("c").is_symlink());
        assert_eq!(f.backups().len(), 2);
        // Restricted permissions noted then applied to the file of the repository.
        let modes =
            parse_modes(&fs::read_to_string(f.repo.join("config/.poste/modes")).unwrap()).unwrap();
        assert_eq!(modes["secret.conf"], 0o600);
        assert_eq!(
            fs::metadata(f.repo.join("config/secret.conf"))
                .unwrap()
                .mode()
                & 0o777,
            0o600
        );
        // Decision on an unknown path: clean refusal.
        let m: HashMap<_, _> = [("zzz".to_string(), Decision::Repo)].into();
        assert!(plan(&f.layout(), &f.env, &m).is_err());
    }

    #[test]
    fn take_home_impossible_on_a_folder_in_place_of_a_file() {
        let f = fx();
        f.cfg("c", "repo");
        fs::create_dir(f.home("c")).unwrap();
        let m: HashMap<_, _> = [("c".to_string(), Decision::Home)].into();
        assert!(plan(&f.layout(), &f.env, &m).is_err());
        // --take-repo is possible: the folder is moved into the backup.
        fs::write(f.home("c/inside"), "z").unwrap();
        f.apply(&f.plan(&[("c", Decision::Repo)]));
        assert!(f.home("c").is_symlink());
        let b = f.backups();
        assert!(f.env.backup_dir().join(&b[0]).join("inside").is_file());
    }

    #[test]
    fn link_elsewhere_is_a_conflict_then_decided() {
        let f = fx();
        let src = f.cfg("c", "repo");
        f.put("other", "other");
        symlink(f.home("other"), f.home("c")).unwrap();
        assert_eq!(f.plan(&[]).conflicts().len(), 1);
        f.apply(&f.plan(&[("c", Decision::Repo)]));
        assert_eq!(fs::read_link(f.home("c")).unwrap(), src);
        assert_eq!(fs::read_to_string(f.home("other")).unwrap(), "other");
        assert_eq!(f.backups().len(), 1);
    }

    #[test]
    fn config_symbolic_links() {
        let f = fx();
        symlink("../../.agents/skills/x", {
            let p = f.repo.join("config/.claude/skills");
            fs::create_dir_all(&p).unwrap();
            p.join("x")
        })
        .unwrap();
        f.apply(&f.plan(&[]));
        // Same target, kept as is.
        assert_eq!(
            fs::read_link(f.home(".claude/skills/x")).unwrap(),
            PathBuf::from("../../.agents/skills/x")
        );
        assert_eq!(f.plan(&[]).writes(), 0);
        // A link elsewhere is a conflict; --take-home takes its target back into the repository.
        fs::remove_file(f.home(".claude/skills/x")).unwrap();
        symlink("/elsewhere-link", f.home(".claude/skills/x")).unwrap();
        assert_eq!(f.plan(&[]).conflicts().len(), 1);
        f.apply(&f.plan(&[(".claude/skills/x", Decision::Home)]));
        assert_eq!(
            fs::read_link(f.repo.join("config/.claude/skills/x")).unwrap(),
            PathBuf::from("/elsewhere-link")
        );
        assert_eq!(f.plan(&[]).writes(), 0);
    }

    #[test]
    fn folder_linked_as_a_block() {
        let f = fx();
        f.cfg(".config/p/k/a.qml", "A");
        f.cfg(".config/p/k/bin/run", "R");
        f.meta("link-dirs", ".config/p/k\n");
        let l = f.layout();
        assert_eq!(l.entries.len(), 1);
        assert_eq!(l.entries[0].kind, Kind::Dir);
        // Target absent: folder link.
        f.apply(&f.plan(&[]));
        assert!(f.home(".config/p/k").is_symlink());
        assert_eq!(
            fs::read_to_string(f.home(".config/p/k/bin/run")).unwrap(),
            "R"
        );
        assert_eq!(f.plan(&[]).writes(), 0);

        // Identical real folder (except __pycache__): moved into the backup, replaced by the link.
        let g = fx();
        g.cfg(".config/p/k/a.qml", "A");
        g.meta("link-dirs", ".config/p/k\n");
        g.put(".config/p/k/a.qml", "A");
        g.put(".config/p/k/__pycache__/x.pyc", "zzz");
        assert_eq!(g.state(".config/p/k"), State::Identical);
        g.apply(&g.plan(&[]));
        assert!(g.home(".config/p/k").is_symlink());
        let b = g.backups();
        assert_eq!(b.len(), 1);
        assert!(g
            .env
            .backup_dir()
            .join(&b[0])
            .join("__pycache__/x.pyc")
            .is_file());

        // Extra real file: conflict, nothing moves.
        let h = fx();
        h.cfg(".config/p/k/a.qml", "A");
        h.meta("link-dirs", ".config/p/k\n");
        h.put(".config/p/k/a.qml", "A");
        h.put(".config/p/k/state.json", "{}");
        assert_eq!(h.state(".config/p/k"), State::Differs);
        h.apply(&h.plan(&[]));
        assert!(!h.home(".config/p/k").is_symlink());
        assert!(h.home(".config/p/k/state.json").is_file());
        // --take-home: the extra file enters the repository.
        h.apply(&h.plan(&[(".config/p/k", Decision::Home)]));
        assert!(h.home(".config/p/k").is_symlink());
        assert_eq!(
            fs::read_to_string(h.repo.join("config/.config/p/k/state.json")).unwrap(),
            "{}"
        );
    }

    #[test]
    fn invalid_link_dirs() {
        let f = fx();
        f.cfg("a/b", "x");
        f.meta("link-dirs", "absent\n");
        assert!(load(&f.repo, &f.env).is_err());
        f.meta("link-dirs", "a\na/b\n");
        assert!(load(&f.repo, &f.env).is_err());
    }

    #[test]
    fn permissions_applied_again_to_the_real_target() {
        let f = fx();
        let src = f.cfg(".config/foot/foot.ini", "x");
        f.cfg(".config/priv/f", "y");
        fs::create_dir_all(f.home(".config/priv")).unwrap();
        f.meta("modes", ".config/foot/foot.ini 0600\n.config/priv 0700\n");
        let p = f.plan(&[]);
        assert_eq!(p.modes.len(), 2);
        f.apply(&p);
        // The mode is on the file of the repository (target of the link), not on the link.
        assert_eq!(fs::metadata(&src).unwrap().mode() & 0o777, 0o600);
        assert_eq!(
            fs::metadata(f.home(".config/foot/foot.ini"))
                .unwrap()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(f.home(".config/priv")).unwrap().mode() & 0o777,
            0o700
        );
        assert!(f.plan(&[]).modes.is_empty());
    }

    #[test]
    fn add_moves_and_links_with_backup() {
        let f = fx();
        let h = f.put(".config/app/conf", "value");
        fs::set_permissions(&h, fs::Permissions::from_mode(0o600)).unwrap();
        let mf = f.env.manifest_file();
        add(&f.env, &f.repo, ".config/app/conf", false, &mf).unwrap();
        let in_repo = f.repo.join("config/.config/app/conf");
        assert_eq!(fs::read_to_string(&in_repo).unwrap(), "value");
        assert_eq!(fs::read_link(&h).unwrap(), in_repo);
        assert_eq!(f.backups().len(), 1);
        let modes =
            parse_modes(&fs::read_to_string(f.repo.join("config/.poste/modes")).unwrap()).unwrap();
        assert_eq!(modes[".config/app/conf"], 0o600);
        // Already managed: no effect, no error.
        add(&f.env, &f.repo, ".config/app/conf", false, &mf).unwrap();
        assert_eq!(f.backups().len(), 1);
        assert_eq!(f.plan(&[]).writes(), 0);
    }

    #[test]
    fn add_refuses_secrets_and_respects_dry_run() {
        let f = fx();
        f.put(".ssh/id_ed25519", "key");
        f.put(".config/app/.env.local", "x");
        f.put(".claude/.credentials.json", "{}");
        f.put(".npmrc", "x");
        let mf = f.env.manifest_file();
        for p in [
            ".ssh/id_ed25519",
            ".ssh",
            ".config/app/.env.local",
            ".claude/.credentials.json",
            ".npmrc",
        ] {
            let e = add(&f.env, &f.repo, p, false, &mf).unwrap_err().to_string();
            assert!(e.contains("refus"), "{p}: {e}");
        }
        assert!(!f.repo.join("config/.ssh").exists());
        // Rule added in the `ignore` file.
        f.meta("ignore", ".config/other/**\n");
        f.put(".config/other/f", "x");
        assert!(add(&f.env, &f.repo, ".config/other/f", false, &mf).is_err());
        // --dry-run: nothing written.
        f.put(".config/ok", "x");
        add(&f.env, &f.repo, ".config/ok", true, &mf).unwrap();
        assert!(!f.repo.join("config/.config/ok").exists());
        assert!(!f.home(".config/ok").is_symlink());
        assert!(!mf.exists());
        // A folder: the ignored files are skipped, the others added.
        f.put(".config/d/a", "1");
        f.put(".config/d/.env", "secret");
        add(&f.env, &f.repo, ".config/d", false, &mf).unwrap();
        assert!(f.repo.join("config/.config/d/a").is_file());
        assert!(!f.repo.join("config/.config/d/.env").exists());
        assert!(f.home(".config/d/.env").is_file() && !f.home(".config/d/.env").is_symlink());
    }

    #[test]
    fn forget_puts_back_an_ordinary_file() {
        let f = fx();
        let src = f.cfg("c", "v");
        f.apply(&f.plan(&[]));
        assert!(f.home("c").is_symlink());
        let mf = f.env.manifest_file();
        forget(&f.layout(), &f.env, "c", &mf).unwrap();
        assert!(!f.home("c").is_symlink());
        assert_eq!(fs::read_to_string(f.home("c")).unwrap(), "v");
        // The repository keeps its file; what follows would see it as identical.
        assert!(src.is_file());
        assert_eq!(f.state("c"), State::Identical);
        assert!(Manifest::load(&mf).unwrap().get(&f.home("c")).is_none());
        // Not linked: refusal.
        assert!(forget(&f.layout(), &f.env, "c", &mf).is_err());
    }

    #[test]
    fn paths_given_by_the_user() {
        let f = fx();
        assert_eq!(rel_of(&f.env, ".config/a").unwrap(), ".config/a");
        assert_eq!(rel_of(&f.env, "~/.config/a").unwrap(), ".config/a");
        let abs = format!("{}/.config/a", f.env.home.display());
        assert_eq!(rel_of(&f.env, &abs).unwrap(), ".config/a");
        assert!(rel_of(&f.env, "/etc/passwd").is_err());
        assert!(rel_of(&f.env, "~/../x").is_err());
    }

    #[test]
    fn diff_reports_differences() {
        let f = fx();
        f.cfg("a", "1\n");
        f.cfg("b", "1\n");
        f.put("a", "2\n");
        f.put("b", "1\n");
        assert!(diff(&f.layout(), &f.env, Some("a")).unwrap());
        assert!(!diff(&f.layout(), &f.env, Some("b")).unwrap());
        assert!(diff(&f.layout(), &f.env, None).unwrap());
        assert!(diff(&f.layout(), &f.env, Some("zzz")).is_err());
    }

    #[test]
    fn status_changes_nothing() {
        let f = fx();
        f.cfg("a", "1");
        f.cfg("b", "1");
        f.put("b", "2");
        let m = Manifest::default();
        print_status(&f.layout(), &f.env, &m).unwrap();
        assert!(!f.home("a").exists() && !f.home("b").is_symlink());
        assert!(f.backups().is_empty());
    }
}
