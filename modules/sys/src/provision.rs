//! Setup of the machine described by `bootstrap/`: packages, system steps (printed,
//! never run), user steps (done by poste after confirmation), manual steps.
//!
//! No step runs `sudo`. The data is in `bootstrap/packages.toml` and
//! `bootstrap/steps.toml`; the contents of root files in `bootstrap/system/`.

use anyhow::{bail, Context, Result};
use poste_core::{time, Config, Env};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Variable reserved for tests: program (without arguments) run in place of `pacman -Qq`.
pub const TEST_PACMAN_VAR: &str = "POSTE_TEST_PACMAN_QQ";

// ------------------------------------------------------------------ data

#[derive(Debug, Default, Deserialize)]
struct PackagesFile {
    #[serde(default)]
    packages: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Steps {
    #[serde(default)]
    plugin: Vec<Plugin>,
    #[serde(default)]
    file: Vec<FileStep>,
    #[serde(default)]
    user: Vec<UserStep>,
    #[serde(default)]
    system: Vec<SystemStep>,
    #[serde(default)]
    manual: Vec<ManualStep>,
}

#[derive(Debug, Deserialize)]
struct Plugin {
    id: String,
    url: String,
    commit: String,
    patch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileStep {
    id: String,
    title: String,
    target: String,
    source: String,
    #[serde(default)]
    json: bool,
}

#[derive(Debug, Deserialize)]
struct UserStep {
    id: String,
    title: String,
    requires: Option<String>,
    #[serde(default)]
    check: Vec<String>,
    #[serde(default)]
    trigger: Vec<String>,
    #[serde(default)]
    run: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct SystemStep {
    id: String,
    title: String,
    requires: Option<String>,
    #[serde(default)]
    when: Vec<String>,
    #[serde(default)]
    check: Vec<String>,
    file: Option<String>,
    source: Option<String>,
    line: Option<String>,
    contains: Option<String>,
    #[serde(default)]
    after: Vec<String>,
    #[serde(default)]
    commands: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ManualStep {
    id: String,
    title: String,
    #[serde(default)]
    check: Vec<String>,
}

/// Context: paths and substitutions (`@HOME@`, `@USER@`, `@UID@`, `@GID@`, `@REPO@`,
/// `@BOOTSTRAP@`, and `@DATA@`: the machine's data repository, empty without it).
pub struct Ctx<'a> {
    env: &'a Env,
    dir: PathBuf,
    /// `bootstrap/` of the data repository, if declared and existing.
    data_dir: Option<PathBuf>,
    /// `bootstrap/` of each add-on listed in `addons`, when it has one.
    addon_dirs: Vec<PathBuf>,
    vars: Vec<(&'static str, String)>,
}

impl<'a> Ctx<'a> {
    pub fn new(env: &'a Env, repo: &Path) -> Self {
        let dir = repo.join("bootstrap");
        let cfg = poste_core::Config::load(env).ok();
        let data = cfg.as_ref().and_then(|c| c.data_path(env));
        let addon_dirs = cfg
            .map(|c| c.addons)
            .unwrap_or_default()
            .iter()
            .filter(|a| poste_core::modules::valid_name(a))
            .map(|a| repo.join("addons").join(a).join("bootstrap"))
            .filter(|d| d.is_dir())
            .collect();
        let data_dir = data
            .as_ref()
            .map(|d| d.join("bootstrap"))
            .filter(|d| d.is_dir());
        let id = |flag: &str| {
            Command::new("id")
                .arg(flag)
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default()
        };
        let vars = vec![
            ("@HOME@", env.home.to_string_lossy().into_owned()),
            ("@USER@", id("-un")),
            ("@UID@", id("-u")),
            ("@GID@", id("-g")),
            ("@REPO@", repo.to_string_lossy().into_owned()),
            ("@BOOTSTRAP@", dir.to_string_lossy().into_owned()),
            (
                "@DATA@",
                data.map(|d| d.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
        ];
        Self {
            env,
            dir,
            data_dir,
            addon_dirs,
            vars,
        }
    }

    fn sub(&self, s: &str) -> String {
        self.vars
            .iter()
            .fold(s.to_string(), |a, (k, v)| a.replace(k, v))
    }

    fn sub_cmd(&self, c: &[String]) -> Vec<String> {
        c.iter().map(|a| self.sub(a)).collect()
    }

    fn read_source(&self, rel: &str) -> Result<String> {
        let p = self.dir.join(rel);
        let s =
            std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        Ok(self.sub(&s))
    }

    fn plugin_dir(&self, id: &str) -> PathBuf {
        self.env.config_home.join("omarchy/plugins").join(id)
    }
}

fn read_steps(dir: &Path) -> Result<Steps> {
    let p = dir.join("steps.toml");
    if !p.is_file() {
        return Ok(Steps::default());
    }
    let s = std::fs::read_to_string(&p)?;
    toml::from_str(&s).with_context(|| format!("reading {}", p.display()))
}

/// The steps of the repository, then those of the listed add-ons, then those of the data
/// repository: their relative paths (patch, source, line) start from their own `bootstrap/`, and
/// a step with the same `id` replaces the earlier one.
fn load_steps(ctx: &Ctx) -> Result<Steps> {
    let mut steps = read_steps(&ctx.dir)?;
    for dd in ctx.addon_dirs.iter().chain(&ctx.data_dir) {
        merge_steps(&mut steps, read_steps(dd)?, dd);
    }
    Ok(steps)
}

fn merge_steps(steps: &mut Steps, more: Steps, dd: &Path) {
    let abs = |r: &mut String| *r = dd.join(&*r).to_string_lossy().into_owned();
    fn put<T>(list: &mut Vec<T>, item: T, id: impl Fn(&T) -> &str) {
        match list.iter().position(|x| id(x) == id(&item)) {
            Some(i) => list[i] = item,
            None => list.push(item),
        }
    }
    for mut p in more.plugin {
        p.patch.as_mut().map(abs);
        put(&mut steps.plugin, p, |x| &x.id);
    }
    for mut f in more.file {
        abs(&mut f.source);
        put(&mut steps.file, f, |x| &x.id);
    }
    for u in more.user {
        put(&mut steps.user, u, |x| &x.id);
    }
    for mut s in more.system {
        s.source.as_mut().map(abs);
        s.line.as_mut().map(abs);
        put(&mut steps.system, s, |x| &x.id);
    }
    for m in more.manual {
        put(&mut steps.manual, m, |x| &x.id);
    }
}

fn read_packages(dir: &Path) -> Result<BTreeMap<String, String>> {
    let p = dir.join("packages.toml");
    if !p.is_file() {
        return Ok(BTreeMap::new());
    }
    let s = std::fs::read_to_string(&p)?;
    let f: PackagesFile =
        toml::from_str(&s).with_context(|| format!("reading {}", p.display()))?;
    Ok(f.packages)
}

/// The packages of the repository, plus those of the listed add-ons and of the data repository.
fn load_packages(ctx: &Ctx) -> Result<BTreeMap<String, String>> {
    let mut out = read_packages(&ctx.dir)?;
    for dd in ctx.addon_dirs.iter().chain(&ctx.data_dir) {
        out.extend(read_packages(dd)?);
    }
    Ok(out)
}

// ------------------------------------------------------------------ skip

/// What `skip` (config.toml) removed from the bootstrap of this machine.
#[derive(Debug, Default, PartialEq)]
pub struct Skipped {
    /// Removed identifiers (those of `skip` that exist, then the steps that depend on them).
    pub ignored: Vec<String>,
    /// Identifiers of `skip` that are neither a package nor a step (typo?).
    pub unknown: Vec<String>,
}

/// Removes from the packages and steps what `skip` designates. A user step with a
/// `trigger` that is skipped is skipped in turn (it assumes that step: recoloring an absent
/// plugin would fail), and so on.
fn apply_skip(
    skip: &[String],
    wanted: &mut BTreeMap<String, String>,
    steps: &mut Steps,
) -> Skipped {
    let mut out = Skipped::default();
    let mut gone: HashSet<String> = HashSet::new();
    for id in skip {
        if gone.contains(id) {
            continue; // doublon
        }
        let known = wanted.remove(id).is_some() | step_ids(steps).contains(id.as_str());
        if known {
            if gone.insert(id.clone()) {
                out.ignored.push(id.clone());
            }
        } else if !out.unknown.contains(id) {
            out.unknown.push(id.clone());
        }
    }
    steps.plugin.retain(|s| !gone.contains(&s.id));
    steps.file.retain(|s| !gone.contains(&s.id));
    steps.system.retain(|s| !gone.contains(&s.id));
    steps.manual.retain(|s| !gone.contains(&s.id));
    steps.user.retain(|s| !gone.contains(&s.id));
    loop {
        let dependent: Vec<String> = steps
            .user
            .iter()
            .filter(|u| u.trigger.iter().any(|t| gone.contains(t)))
            .map(|u| u.id.clone())
            .collect();
        if dependent.is_empty() {
            break;
        }
        steps.user.retain(|u| !dependent.contains(&u.id));
        for d in dependent {
            gone.insert(d.clone());
            out.ignored.push(d);
        }
    }
    out
}

fn step_ids(steps: &Steps) -> HashSet<&str> {
    steps
        .plugin
        .iter()
        .map(|s| s.id.as_str())
        .chain(steps.file.iter().map(|s| s.id.as_str()))
        .chain(steps.user.iter().map(|s| s.id.as_str()))
        .chain(steps.system.iter().map(|s| s.id.as_str()))
        .chain(steps.manual.iter().map(|s| s.id.as_str()))
        .collect()
}

/// Wanted packages and steps, the machine's `skip` applied; warns about unknown identifiers.
fn load_wanted(ctx: &Ctx) -> Result<(BTreeMap<String, String>, Steps, Skipped)> {
    let cfg = Config::load(ctx.env)?;
    let mut wanted = load_packages(ctx)?;
    let mut steps = load_steps(ctx)?;
    let sk = apply_skip(&cfg.skip, &mut wanted, &mut steps);
    for u in &sk.unknown {
        eprintln!(
            "warning: '{u}' (config.toml, skip) is neither a package of packages.toml nor the id of a step of steps.toml (typo?)"
        );
    }
    Ok((wanted, steps, sk))
}

/// Kind of each identifier that `skip` can designate (bootstrap packages and steps).
pub fn skip_catalog(env: &Env, repo: &Path) -> Result<BTreeMap<String, &'static str>> {
    let ctx = Ctx::new(env, repo);
    let steps = load_steps(&ctx)?;
    let mut out: BTreeMap<String, &'static str> = BTreeMap::new();
    for k in load_packages(&ctx)?.into_keys() {
        out.insert(k, "package");
    }
    let kinds: [(Vec<&str>, &'static str); 5] = [
        (
            steps.plugin.iter().map(|s| s.id.as_str()).collect(),
            "plugin",
        ),
        (
            steps.file.iter().map(|s| s.id.as_str()).collect(),
            "file",
        ),
        (
            steps.user.iter().map(|s| s.id.as_str()).collect(),
            "user step",
        ),
        (
            steps.system.iter().map(|s| s.id.as_str()).collect(),
            "system step",
        ),
        (
            steps.manual.iter().map(|s| s.id.as_str()).collect(),
            "manual step",
        ),
    ];
    for (ids, kind) in kinds {
        for id in ids {
            out.entry(id.to_string()).or_insert(kind);
        }
    }
    Ok(out)
}

/// Effect of `skip` on this repository, without a warning (for `poste sys skip`).
pub fn skip_effect(env: &Env, repo: &Path, skip: &[String]) -> Result<Skipped> {
    let ctx = Ctx::new(env, repo);
    let mut wanted = load_packages(&ctx)?;
    let mut steps = load_steps(&ctx)?;
    Ok(apply_skip(skip, &mut wanted, &mut steps))
}

fn print_skipped(sk: &Skipped) {
    if !sk.ignored.is_empty() {
        println!(
            "\nignored on this machine (config.toml, skip): {}",
            sk.ignored.join(", ")
        );
    }
}

// ------------------------------------------------------------------ utilitaires

/// Runs a command without output; true if it ends with code 0.
fn succeeds(cmd: &[String]) -> bool {
    let Some((prog, args)) = cmd.split_first() else {
        return false;
    };
    Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn have(env: &Env, bin: &str) -> bool {
    let in_dirs = |dirs: &[PathBuf]| dirs.iter().any(|d| d.join(bin).is_file());
    // ~/.local/bin too: a setup run without it in its PATH (script, timer) must still find the
    // programs installed there, or it would unplug their add-ons.
    in_dirs(&env.path)
        || env.bin_dir().join(bin).is_file()
        || std::env::var_os("PATH")
            .map(|p| in_dirs(&std::env::split_paths(&p).collect::<Vec<_>>()))
            .unwrap_or(false)
}

fn done_dir(env: &Env) -> PathBuf {
    env.state_dir().join("done")
}

fn is_marked(env: &Env, id: &str) -> bool {
    done_dir(env).join(id).is_file()
}

/// Installed packages (`pacman -Qq`, or the test program).
fn installed_packages() -> Result<HashSet<String>> {
    let out = match std::env::var_os(TEST_PACMAN_VAR) {
        Some(p) if !p.is_empty() => Command::new(p).output(),
        _ => Command::new("pacman").arg("-Qq").output(),
    }
    .context("cannot run pacman -Qq")?;
    if !out.status.success() {
        bail!("pacman -Qq failed");
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Wanted packages that are not installed (alphabetical order).
fn missing_of(wanted: &BTreeMap<String, String>, installed: &HashSet<String>) -> Vec<String> {
    wanted
        .keys()
        .filter(|k| !installed.contains(*k))
        .cloned()
        .collect()
}

// ------------------------------------------------------------------ system steps

#[derive(Debug, PartialEq)]
enum Sys {
    Done,
    Todo(&'static str),
    /// Cannot be checked without root: stays displayed until it is marked done.
    Unknown(&'static str),
    NotApplicable(String),
}

fn sys_state(ctx: &Ctx, s: &SystemStep) -> Sys {
    if let Some(r) = &s.requires {
        if !have(ctx.env, r) {
            return Sys::NotApplicable(format!("{r} absent"));
        }
    }
    if !s.when.is_empty() && !succeeds(&ctx.sub_cmd(&s.when)) {
        return Sys::NotApplicable("not applicable on this machine".into());
    }
    let verified = if let (Some(file), Some(src)) = (&s.file, &s.source) {
        let want = match ctx.read_source(src) {
            Ok(w) => w,
            Err(_) => return Sys::Unknown("source illisible"),
        };
        match std::fs::read_to_string(ctx.sub(file)) {
            Ok(c) if c.trim_end() == want.trim_end() => Sys::Done,
            Ok(_) => Sys::Todo("different content"),
            Err(e) if e.kind() == ErrorKind::NotFound => Sys::Todo("file missing"),
            Err(_) => Sys::Unknown("unreadable without root"),
        }
    } else if let (Some(file), Some(needle)) = (&s.file, &s.contains) {
        match std::fs::read_to_string(ctx.sub(file)) {
            Ok(c) if c.contains(&ctx.sub(needle)) => Sys::Done,
            Ok(_) => Sys::Todo("line missing"),
            Err(e) if e.kind() == ErrorKind::NotFound => Sys::Todo("file missing"),
            Err(_) => Sys::Unknown("unreadable without root"),
        }
    } else if !s.check.is_empty() {
        if succeeds(&ctx.sub_cmd(&s.check)) {
            Sys::Done
        } else {
            Sys::Todo("to do")
        }
    } else {
        Sys::Unknown("cannot be checked")
    };
    match verified {
        Sys::Unknown(_) if is_marked(ctx.env, &s.id) => Sys::Done,
        v => v,
    }
}

/// The lines of the block to copy for a system step.
fn sys_commands(ctx: &Ctx, s: &SystemStep) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if let (Some(file), Some(src)) = (&s.file, &s.source) {
        let body = ctx.read_source(src)?;
        out.push(format!(
            "sudo tee {} >/dev/null <<'EOF'\n{}\nEOF",
            ctx.sub(file),
            body.trim_end()
        ));
    }
    if let (Some(file), Some(src)) = (&s.file, &s.line) {
        let body = ctx.read_source(src)?;
        out.push(format!(
            "sudo tee -a {} >/dev/null <<'EOF'\n{}\nEOF",
            ctx.sub(file),
            body.trim_end()
        ));
    }
    out.extend(s.after.iter().map(|c| ctx.sub(c)));
    out.extend(s.commands.iter().map(|c| ctx.sub(c)));
    Ok(out)
}

// ------------------------------------------------------------------ user steps

#[derive(Debug, Clone)]
enum Act {
    Cmd(Vec<String>),
    Write { target: PathBuf, content: String },
}

impl Act {
    fn show(&self) -> String {
        match self {
            Act::Cmd(c) => c.join(" "),
            Act::Write { target, .. } => format!("write {}", target.display()),
        }
    }
}

#[derive(Debug, Clone)]
struct Todo {
    id: String,
    title: String,
    acts: Vec<Act>,
}

#[derive(Debug, Default)]
pub struct UserPlan {
    todos: Vec<Todo>,
    /// Steps triggered only if one of the cited steps has just been done.
    triggered: Vec<(Vec<String>, Todo)>,
    /// What is ignored, ambiguous or different: shown, never touched.
    notes: Vec<String>,
}

fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let o = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    o.status
        .success()
        .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn git_ok(dir: &Path, args: &[&str]) -> bool {
    git(dir, args).is_some()
}

fn cmd(parts: &[&str]) -> Act {
    Act::Cmd(parts.iter().map(|s| s.to_string()).collect())
}

/// Classifies a third-party plugin: nothing to do (`Ok(vec![])`), acts, or a "by hand" note.
fn plugin_acts(ctx: &Ctx, p: &Plugin) -> Result<Vec<Act>, String> {
    let dir = ctx.plugin_dir(&p.id);
    let patch = p
        .patch
        .as_ref()
        .map(|r| ctx.dir.join(r).to_string_lossy().into_owned());
    let d = dir.to_string_lossy().into_owned();
    let mut acts = Vec::new();
    let checkout = |acts: &mut Vec<Act>| {
        acts.push(cmd(&[
            "git", "-C", &d, "fetch", "--quiet", "--tags", "origin",
        ]));
        acts.push(cmd(&[
            "git",
            "-C",
            &d,
            "-c",
            "advice.detachedHead=false",
            "checkout",
            "--quiet",
            &p.commit,
        ]));
    };
    if !dir.exists() {
        if !have(ctx.env, "omarchy") {
            return Err(format!(
                "{}: absent, and the omarchy command cannot be found",
                p.id
            ));
        }
        acts.push(cmd(&["omarchy", "plugin", "add", &p.url, "--yes"]));
        checkout(&mut acts);
        if let Some(pa) = &patch {
            acts.push(cmd(&["git", "-C", &d, "apply", pa]));
        }
        return Ok(acts);
    }
    if !dir.join(".git").exists() {
        return Err(format!(
            "{}: folder present but without git, nothing is touched",
            p.id
        ));
    }
    let head = git(&dir, &["rev-parse", "HEAD"]).unwrap_or_default();
    if !head.starts_with(&p.commit) {
        if !git(&dir, &["status", "--porcelain", "--untracked-files=no"])
            .unwrap_or_default()
            .is_empty()
        {
            return Err(format!(
                "{}: at {} instead of {} and files modified (patch already applied?): to handle by hand",
                p.id,
                &head[..head.len().min(7)],
                p.commit
            ));
        }
        checkout(&mut acts);
        if let Some(pa) = &patch {
            acts.push(cmd(&["git", "-C", &d, "apply", pa]));
        }
        return Ok(acts);
    }
    if let Some(pa) = &patch {
        if git_ok(&dir, &["apply", "--reverse", "--check", pa]) {
            // already applied
        } else if git_ok(&dir, &["apply", "--check", pa]) {
            acts.push(cmd(&["git", "-C", &d, "apply", pa]));
        } else {
            return Err(format!(
                "{}: the patch no longer applies (plugin updated?): {pa}",
                p.id
            ));
        }
    }
    Ok(acts)
}

fn json_equal(a: &str, b: &str) -> bool {
    match (
        serde_json::from_str::<serde_json::Value>(a),
        serde_json::from_str::<serde_json::Value>(b),
    ) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn user_plan(ctx: &Ctx, steps: &Steps) -> Result<UserPlan> {
    let mut plan = UserPlan::default();
    for p in &steps.plugin {
        match plugin_acts(ctx, p) {
            Ok(acts) if acts.is_empty() => {}
            Ok(acts) => plan.todos.push(Todo {
                id: p.id.clone(),
                title: format!("plugin {} at {}", p.id, p.commit),
                acts,
            }),
            Err(note) => plan.notes.push(note),
        }
    }
    for f in &steps.file {
        let target = PathBuf::from(ctx.sub(&f.target));
        let content = ctx.read_source(&f.source)?;
        match std::fs::read_to_string(&target) {
            Ok(cur) if cur == content || (f.json && json_equal(&cur, &content)) => {}
            Ok(_) => plan.notes.push(format!(
                "{}: {} differs from the repository, left as is (edit or delete it so that it is put back)",
                f.id,
                target.display()
            )),
            Err(e) if e.kind() == ErrorKind::NotFound => plan.todos.push(Todo {
                id: f.id.clone(),
                title: f.title.clone(),
                acts: vec![Act::Write { target, content }],
            }),
            Err(e) => plan.notes.push(format!("{}: {} unreadable: {e}", f.id, target.display())),
        }
    }
    for u in &steps.user {
        if let Some(r) = &u.requires {
            if !have(ctx.env, r) {
                plan.notes.push(format!(
                    "{}: ignored, {r} not found (install the package then run again)",
                    u.id
                ));
                continue;
            }
        }
        let todo = Todo {
            id: u.id.clone(),
            title: u.title.clone(),
            acts: u.run.iter().map(|c| Act::Cmd(ctx.sub_cmd(c))).collect(),
        };
        if !u.trigger.is_empty() {
            plan.triggered.push((u.trigger.clone(), todo));
        } else if u.check.is_empty() || !succeeds(&ctx.sub_cmd(&u.check)) {
            plan.todos.push(todo);
        }
    }
    Ok(plan)
}

fn run_act(act: &Act) -> Result<()> {
    match act {
        Act::Cmd(c) => {
            let (prog, args) = c.split_first().context("empty command")?;
            let st = Command::new(prog)
                .args(args)
                .status()
                .with_context(|| format!("cannot run {prog}"))?;
            if !st.success() {
                bail!("{} failed ({st})", c.join(" "));
            }
            Ok(())
        }
        Act::Write { target, content } => {
            let parent = target.parent().context("target without a parent folder")?;
            std::fs::create_dir_all(parent)?;
            let tmp = parent.join(format!(".poste-tmp-{}", std::process::id()));
            std::fs::write(&tmp, content)?;
            std::fs::rename(&tmp, target)
                .with_context(|| format!("writing {}", target.display()))
        }
    }
}

/// Result of applying the user steps.
#[derive(Debug, Default)]
pub struct Applied {
    pub done: Vec<String>,
    pub failed: Vec<String>,
}

fn apply_user(plan: &UserPlan) -> Applied {
    let mut res = Applied::default();
    let run_todo = |t: &Todo, res: &mut Applied| {
        println!("\n  {}: {}", t.id, t.title);
        for a in &t.acts {
            println!("    $ {}", a.show());
            if let Err(e) = run_act(a) {
                eprintln!("    failure: {e:#}");
                res.failed.push(t.id.clone());
                return;
            }
        }
        res.done.push(t.id.clone());
    };
    for t in &plan.todos {
        run_todo(t, &mut res);
    }
    for (triggers, t) in &plan.triggered {
        if triggers.iter().any(|x| res.done.contains(x)) {
            run_todo(t, &mut res);
        }
    }
    res
}

/// The triggered steps that would run if their triggers are in the plan.
fn triggered_in_plan(plan: &UserPlan) -> Vec<&Todo> {
    plan.triggered
        .iter()
        .filter(|(tr, _)| plan.todos.iter().any(|t| tr.contains(&t.id)))
        .map(|(_, t)| t)
        .collect()
}

// ------------------------------------------------------------------ manuelles

fn manual_pending<'a>(ctx: &Ctx, steps: &'a Steps) -> Vec<&'a ManualStep> {
    steps
        .manual
        .iter()
        .filter(|m| {
            !is_marked(ctx.env, &m.id) && (m.check.is_empty() || !succeeds(&ctx.sub_cmd(&m.check)))
        })
        .collect()
}

fn print_manual(ctx: &Ctx, steps: &Steps) {
    let pending = manual_pending(ctx, steps);
    if pending.is_empty() {
        return;
    }
    println!("\nManual steps left (to do by hand, then `poste sys done <id>`)");
    for m in pending {
        println!("  [{}] {}", m.id, ctx.sub(&m.title));
    }
}

/// `poste sys done [<id>] [--undo]`: marks a manual step (or an unverifiable system step) done.
pub fn done(env: &Env, repo: &Path, id: Option<&str>, undo: bool) -> Result<()> {
    let ctx = Ctx::new(env, repo);
    let steps = load_steps(&ctx)?;
    let known: Vec<(&str, &str)> = steps
        .manual
        .iter()
        .map(|m| (m.id.as_str(), m.title.as_str()))
        .chain(
            steps
                .system
                .iter()
                .filter(|s| {
                    // only those that poste cannot verify
                    s.check.is_empty() && s.contains.is_none() && s.file.is_some()
                })
                .map(|s| (s.id.as_str(), s.title.as_str())),
        )
        .collect();
    let Some(id) = id else {
        println!("Steps that are marked by hand:");
        for (i, t) in &known {
            let mark = if is_marked(env, i) {
                "done"
            } else {
                "to do"
            };
            println!("  [{i}] ({mark}) {}", ctx.sub(t));
        }
        return Ok(());
    };
    if !known.iter().any(|(i, _)| *i == id) {
        bail!(
            "unknown step: {id}. Known: {}",
            known.iter().map(|(i, _)| *i).collect::<Vec<_>>().join(", ")
        );
    }
    let f = done_dir(env).join(id);
    if undo {
        match std::fs::remove_file(&f) {
            Ok(()) => println!("{id}: to do again"),
            Err(e) if e.kind() == ErrorKind::NotFound => println!("{id}: was not marked done"),
            Err(e) => return Err(e.into()),
        }
    } else {
        std::fs::create_dir_all(done_dir(env))?;
        std::fs::write(&f, format!("{}\n", time::now_rfc3339()))?;
        println!("{id}: marked done");
    }
    Ok(())
}

// ------------------------------------------------------------------ affichage commun

struct Packages {
    missing: Vec<String>,
    total: usize,
}

fn packages_report(wanted: &BTreeMap<String, String>) -> Result<Option<Packages>> {
    if wanted.is_empty() {
        return Ok(None);
    }
    let installed = installed_packages()?;
    Ok(Some(Packages {
        missing: missing_of(wanted, &installed),
        total: wanted.len(),
    }))
}

fn print_packages(wanted: &BTreeMap<String, String>) {
    println!("\nPaquets");
    match packages_report(wanted) {
        Err(e) => println!("  check impossible: {e:#}"),
        Ok(None) => println!("  no package declared"),
        Ok(Some(p)) if p.missing.is_empty() => println!("  all present ({})", p.total),
        Ok(Some(p)) => {
            println!(
                "  missing ({}/{}): {}",
                p.missing.len(),
                p.total,
                p.missing.join(" ")
            );
            println!("  To run yourself (poste does not run it):");
            println!("    yay -S --needed {}", p.missing.join(" "));
        }
    }
}

/// Shows the pending system steps; returns their number.
fn print_system(ctx: &Ctx, steps: &Steps) -> Result<usize> {
    let mut block: Vec<String> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for s in &steps.system {
        match sys_state(ctx, s) {
            Sys::Done | Sys::NotApplicable(_) => {}
            st => {
                let (tag, why) = match st {
                    Sys::Todo(w) => ("to do", w),
                    Sys::Unknown(w) => ("to check", w),
                    _ => unreachable!(),
                };
                lines.push(format!(
                    "  [{}] {} ({tag}: {why})",
                    s.id,
                    ctx.sub(&s.title)
                ));
                if tag == "to check" {
                    lines.push(format!(
                        "      if it is already done: poste sys done {}",
                        s.id
                    ));
                }
                block.push(format!("# {}: {}", s.id, ctx.sub(&s.title)));
                block.extend(sys_commands(ctx, s)?);
                block.push(String::new());
            }
        }
    }
    println!("\nSystem steps (sudo: to run yourself, poste never does it)");
    if lines.is_empty() {
        println!("  nothing to do");
        return Ok(0);
    }
    for l in &lines {
        println!("{l}");
    }
    println!("\n  Block to copy:");
    println!("  ----------------------------------------");
    for l in &block {
        println!("{l}");
    }
    println!("  ----------------------------------------");
    Ok(lines.iter().filter(|l| l.starts_with("  [")).count())
}

fn print_user_plan(plan: &UserPlan) {
    println!("\nUser steps (done by poste)");
    if plan.todos.is_empty() {
        println!("  nothing to do");
    }
    for t in &plan.todos {
        println!("  to do  {}: {}", t.id, t.title);
        for a in &t.acts {
            println!("             $ {}", a.show());
        }
    }
    for t in triggered_in_plan(plan) {
        println!("  then   {}: {}", t.id, t.title);
        for a in &t.acts {
            println!("             $ {}", a.show());
        }
    }
    for n in &plan.notes {
        println!("  note     {n}");
    }
}

// ------------------------------------------------------------------ entry points

/// "Bootstrap" phase of `setup`: packages, system, user (with confirmation), manual.
/// Returns the user steps applied and whether one failed.
pub fn setup(
    env: &Env,
    repo: &Path,
    yes: bool,
    dry_run: bool,
    confirm: &dyn Fn(&str) -> Result<bool>,
) -> Result<Applied> {
    let ctx = Ctx::new(env, repo);
    let (wanted, steps, skipped) = load_wanted(&ctx)?;
    print_packages(&wanted);
    print_system(&ctx, &steps)?;
    let plan = user_plan(&ctx, &steps)?;
    print_user_plan(&plan);
    let mut applied = Applied::default();
    let n = plan.todos.len();
    if n > 0 {
        if dry_run {
            println!("\nDry run (--dry-run): user steps not run");
        } else if yes || confirm(&format!("\nRun these {n} user step(s)? [y/N] "))? {
            applied = apply_user(&plan);
        } else {
            println!("User steps not run.");
        }
    }
    print_manual(&ctx, &steps);
    print_skipped(&skipped);
    Ok(applied)
}

/// Lines of `poste sys status` for packages, system, user and manual steps.
pub fn status(env: &Env, repo: &Path) -> Result<()> {
    let ctx = Ctx::new(env, repo);
    let (wanted, steps, skipped) = load_wanted(&ctx)?;
    println!("\nPaquets");
    match packages_report(&wanted) {
        Err(e) => println!("  check impossible: {e:#}"),
        Ok(None) => println!("  no package declared"),
        Ok(Some(p)) if p.missing.is_empty() => println!("  all present ({})", p.total),
        Ok(Some(p)) => {
            println!("  missing ({}/{}):", p.missing.len(), p.total);
            for m in &p.missing {
                println!("    {m}");
            }
        }
    }
    println!("\nSystem steps");
    let mut any = false;
    for s in &steps.system {
        match sys_state(&ctx, s) {
            Sys::Todo(w) => {
                any = true;
                println!("  to do        [{}] {} ({w})", s.id, ctx.sub(&s.title));
            }
            Sys::Unknown(w) => {
                any = true;
                println!("  to check     [{}] {} ({w})", s.id, ctx.sub(&s.title));
            }
            _ => {}
        }
    }
    if !any {
        println!("  nothing pending");
    }
    let plan = user_plan(&ctx, &steps)?;
    println!("\nUser steps");
    if plan.todos.is_empty() && plan.notes.is_empty() {
        println!("  nothing pending");
    }
    for t in &plan.todos {
        println!("  to do  [{}] {}", t.id, t.title);
    }
    for n in &plan.notes {
        println!("  note     {n}");
    }
    let manual = manual_pending(&ctx, &steps);
    println!("\nManual steps");
    if manual.is_empty() {
        println!("  toutes faites");
    }
    for m in manual {
        println!("  to do  [{}] {}", m.id, ctx.sub(&m.title));
    }
    print_skipped(&skipped);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> (tempfile::TempDir, PathBuf, Env) {
        let d = tempfile::tempdir().unwrap();
        let repo = d.path().join("repo");
        fs::create_dir_all(repo.join("bootstrap/system")).unwrap();
        fs::create_dir_all(repo.join("bootstrap/patches")).unwrap();
        let home = d.path().join("home");
        fs::create_dir_all(&home).unwrap();
        (d, repo, Env::with_home(&home))
    }

    fn write(repo: &Path, rel: &str, s: &str) {
        let p = repo.join("bootstrap").join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, s).unwrap();
    }

    fn sh(dir: &Path, args: &[&str]) {
        let st = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .status()
            .unwrap();
        assert!(st.success(), "git {args:?}");
    }

    #[test]
    fn missing_packages() {
        let wanted: BTreeMap<String, String> = [("a", ""), ("b", ""), ("c", "")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let have: HashSet<String> = ["a", "c", "z"].iter().map(|s| s.to_string()).collect();
        assert_eq!(missing_of(&wanted, &have), vec!["b"]);
    }

    #[test]
    fn steps_and_packages_of_the_data_repository() {
        let (d, repo, env) = fixture();
        write(&repo, "packages.toml", "[packages]\na = \"\"\n");
        write(
            &repo,
            "steps.toml",
            "[[system]]\nid = \"s\"\ntitle = \"exemple\"\ncheck = [\"true\"]\n\
             [[manual]]\nid = \"m\"\ntitle = \"m\"\n",
        );
        let data = d.path().join("data");
        let dw = |rel: &str, s: &str| {
            let p = data.join("bootstrap").join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, s).unwrap();
        };
        dw("packages.toml", "[packages]\nb = \"\"\n");
        dw("system/x.line", "LINE @DATA@\n");
        dw(
            "steps.toml",
            "[[system]]\nid = \"s\"\ntitle = \"real\"\nfile = \"/f\"\nline = \"system/x.line\"\n\
             [[manual]]\nid = \"n\"\ntitle = \"n\"\n",
        );
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(
            env.config_file(),
            format!("data = \"{}\"\n", data.display()),
        )
        .unwrap();

        let ctx = Ctx::new(&env, &repo);
        let pk: Vec<_> = load_packages(&ctx).unwrap().into_keys().collect();
        assert_eq!(pk, ["a", "b"]);
        let steps = load_steps(&ctx).unwrap();
        // Same id: the step of the data repository replaces the one of the repository.
        assert_eq!(steps.system.len(), 1);
        assert_eq!(steps.system[0].title, "real");
        let ids: Vec<_> = steps.manual.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["m", "n"]);
        // Source read in the bootstrap/ of the data repository, @DATA@ replaced.
        let cmds = sys_commands(&ctx, &steps.system[0]).unwrap();
        assert!(
            cmds[0].contains(&format!("LINE {}", data.display())),
            "{cmds:?}"
        );
    }

    #[test]
    fn steps_and_packages_of_chosen_addons_only() {
        let (_d, repo, env) = fixture();
        write(&repo, "packages.toml", "[packages]\na = \"\"\n");
        for name in ["chosen", "other"] {
            let b = repo.join("addons").join(name).join("bootstrap");
            fs::create_dir_all(&b).unwrap();
            fs::write(
                b.join("packages.toml"),
                format!("[packages]\n{name}-lib = \"\"\n"),
            )
            .unwrap();
            fs::write(
                b.join("steps.toml"),
                format!("[[user]]\nid = \"{name}\"\ntitle = \"install\"\nrun = [[\"true\"]]\n"),
            )
            .unwrap();
        }
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "addons = [\"chosen\", \"../x\"]\n").unwrap();

        let ctx = Ctx::new(&env, &repo);
        let pk: Vec<_> = load_packages(&ctx).unwrap().into_keys().collect();
        assert_eq!(pk, ["a", "chosen-lib"]);
        let steps = load_steps(&ctx).unwrap();
        let ids: Vec<_> = steps.user.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(ids, ["chosen"]);
    }

    #[test]
    fn system_state_file_line_and_marker() {
        let (_d, repo, env) = fixture();
        write(&repo, "system/r.rules", "rule for @HOME@\n");
        write(&repo, "system/l.line", "LINE-X\n");
        let ctx = Ctx::new(&env, &repo);
        let target = env.home.join("etc-r");
        let fstab = env.home.join("fstab");
        let steps: Steps = toml::from_str(&format!(
            r#"
[[system]]
id = "r"
title = "r"
file = "{t}"
source = "system/r.rules"
[[system]]
id = "l"
title = "l"
file = "{f}"
line = "system/l.line"
contains = "LINE-X"
after = ["sudo true"]
[[system]]
id = "c"
title = "c"
check = ["false"]
[[system]]
id = "w"
title = "w"
when = ["false"]
check = ["false"]
"#,
            t = target.display(),
            f = fstab.display()
        ))
        .unwrap();
        let st = |i: usize| sys_state(&ctx, &steps.system[i]);
        assert_eq!(st(0), Sys::Todo("file missing"));
        fs::write(&target, "other").unwrap();
        assert_eq!(st(0), Sys::Todo("different content"));
        fs::write(&target, format!("rule for {}\n", env.home.display())).unwrap();
        assert_eq!(st(0), Sys::Done);
        fs::write(&fstab, "x\n").unwrap();
        assert_eq!(st(1), Sys::Todo("line missing"));
        fs::write(&fstab, "x\nLINE-X\n").unwrap();
        assert_eq!(st(1), Sys::Done);
        assert_eq!(st(2), Sys::Todo("to do"));
        assert!(matches!(st(3), Sys::NotApplicable(_)));
        let cmds = sys_commands(&ctx, &steps.system[0]).unwrap();
        assert!(cmds[0].starts_with("sudo tee ") && cmds[0].contains("<<'EOF'"));
        assert_eq!(
            sys_commands(&ctx, &steps.system[1]).unwrap()[1],
            "sudo true"
        );
        // Unreadable: unknown, then done once marked.
        let p = env.home.join("secret");
        fs::write(&p, "x").unwrap();
        fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o000)).unwrap();
        if fs::read_to_string(&p).is_err() {
            let s2: Steps = toml::from_str(&format!(
                "[[system]]\nid=\"u\"\ntitle=\"u\"\nfile=\"{}\"\nsource=\"system/r.rules\"\n",
                p.display()
            ))
            .unwrap();
            assert!(matches!(sys_state(&ctx, &s2.system[0]), Sys::Unknown(_)));
            fs::create_dir_all(done_dir(&env)).unwrap();
            fs::write(done_dir(&env).join("u"), "x").unwrap();
            assert_eq!(sys_state(&ctx, &s2.system[0]), Sys::Done);
        }
    }

    #[test]
    fn mark_done_and_undo() {
        let (_d, repo, env) = fixture();
        write(
            &repo,
            "steps.toml",
            "[[manual]]\nid = \"m1\"\ntitle = \"faire m1\"\n[[manual]]\nid = \"m2\"\ntitle = \"m2\"\ncheck = [\"true\"]\n",
        );
        let ctx = Ctx::new(&env, &repo);
        let steps = load_steps(&ctx).unwrap();
        assert_eq!(
            manual_pending(&ctx, &steps).len(),
            1,
            "m2 checked by its command"
        );
        assert!(done(&env, &repo, Some("zzz"), false).is_err());
        done(&env, &repo, Some("m1"), false).unwrap();
        assert!(manual_pending(&ctx, &steps).is_empty());
        done(&env, &repo, Some("m1"), true).unwrap();
        assert_eq!(manual_pending(&ctx, &steps).len(), 1);
    }

    #[test]
    fn plugin_commit_patch_and_file() {
        let (_d, repo, env) = fixture();
        let ctx = Ctx::new(&env, &repo);
        let dir = ctx.plugin_dir("p");
        fs::create_dir_all(&dir).unwrap();
        sh(&dir, &["init", "-q"]);
        fs::write(dir.join("f.txt"), "one\n").unwrap();
        sh(&dir, &["add", "."]);
        sh(&dir, &["commit", "-q", "-m", "1"]);
        let head = git(&dir, &["rev-parse", "--short", "HEAD"]).unwrap();
        // Patch: changes "one" into "two".
        write(
            &repo,
            "patches/p.patch",
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-one\n+two\n",
        );
        let plug = |commit: &str| Plugin {
            id: "p".into(),
            url: "u".into(),
            commit: commit.into(),
            patch: Some("patches/p.patch".into()),
        };
        // Right commit, patch to apply.
        let acts = plugin_acts(&ctx, &plug(&head)).unwrap();
        assert_eq!(acts.len(), 1);
        run_act(&acts[0]).unwrap();
        assert_eq!(fs::read_to_string(dir.join("f.txt")).unwrap(), "two\n");
        // Already applied: nothing.
        assert!(plugin_acts(&ctx, &plug(&head)).unwrap().is_empty());
        // Other commit wanted and tree modified: note, nothing to do.
        assert!(plugin_acts(&ctx, &plug("deadbee")).is_err());
        // Patch that no longer applies.
        fs::write(dir.join("f.txt"), "three\n").unwrap();
        assert!(plugin_acts(&ctx, &plug(&head)).is_err());
        // Absent plugin without omarchy: note.
        let mut p = plug(&head);
        p.id = "absent".into();
        assert_eq!(
            plugin_acts(&ctx, &p).is_ok(),
            have(&env, "omarchy"),
            "absent: omarchy plugin add if omarchy exists, otherwise a note"
        );

        // JSON file: absent -> written; same JSON formatted otherwise -> nothing; different -> note.
        write(&repo, "files/c.json", "{\"a\": 1}\n");
        let target = env.home.join("x/c.json");
        let steps: Steps = toml::from_str(&format!(
            "[[file]]\nid=\"c\"\ntitle=\"c\"\ntarget=\"{}\"\nsource=\"files/c.json\"\njson=true\n",
            target.display()
        ))
        .unwrap();
        let plan = user_plan(&ctx, &steps).unwrap();
        assert_eq!(plan.todos.len(), 1);
        apply_user(&plan);
        assert!(target.is_file());
        fs::write(&target, "{\n \"a\":1 }").unwrap();
        let plan = user_plan(&ctx, &steps).unwrap();
        assert!(plan.todos.is_empty() && plan.notes.is_empty());
        fs::write(&target, "{\"a\": 2}").unwrap();
        let plan = user_plan(&ctx, &steps).unwrap();
        assert!(plan.todos.is_empty() && plan.notes.len() == 1);
    }

    #[test]
    fn user_steps_check_and_trigger() {
        let (_d, repo, env) = fixture();
        let ctx = Ctx::new(&env, &repo);
        let marker = env.home.join("marker");
        let steps: Steps = toml::from_str(&format!(
            r#"
[[user]]
id = "done"
title = "already done"
check = ["true"]
run = [["false"]]
[[user]]
id = "a"
title = "a"
check = ["false"]
run = [["touch", "{m}"]]
[[user]]
id = "after"
title = "after a"
trigger = ["a"]
run = [["true"]]
[[user]]
id = "no-bin"
title = "x"
requires = "binary-that-does-not-exist"
run = [["false"]]
"#,
            m = marker.display()
        ))
        .unwrap();
        let plan = user_plan(&ctx, &steps).unwrap();
        assert_eq!(plan.todos.len(), 1);
        assert_eq!(plan.notes.len(), 1);
        assert_eq!(triggered_in_plan(&plan).len(), 1);
        let r = apply_user(&plan);
        assert_eq!(r.done, vec!["a", "after"]);
        assert!(r.failed.is_empty() && marker.is_file());
    }

    #[test]
    fn skip_removes_packages_steps_and_dependents() {
        let mut steps: Steps = toml::from_str(
            r#"
[[plugin]]
id = "pl"
url = "u"
commit = "c"
[[file]]
id = "cfg"
title = "t"
target = "/x"
source = "s"
[[user]]
id = "recolor"
title = "t"
trigger = ["cfg", "pl"]
[[user]]
id = "after-recolor"
title = "t"
trigger = ["recolor"]
[[user]]
id = "other"
title = "t"
trigger = ["cfg"]
[[user]]
id = "free"
title = "t"
[[system]]
id = "sy"
title = "t"
[[manual]]
id = "ma"
title = "t"
"#,
        )
        .unwrap();
        let mut wanted: BTreeMap<String, String> = [("gimp", ""), ("vlc", "")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let skip: Vec<String> = ["gimp", "pl", "sy", "ma", "gimp", "typo", "typo"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let sk = apply_skip(&skip, &mut wanted, &mut steps);
        assert_eq!(wanted.keys().collect::<Vec<_>>(), ["vlc"]);
        assert!(steps.plugin.is_empty() && steps.system.is_empty() && steps.manual.is_empty());
        let users: Vec<&str> = steps.user.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(users, ["other", "free"], "other depends on cfg, not skipped");
        assert_eq!(steps.file.len(), 1, "cfg is not skipped");
        assert_eq!(
            sk.ignored,
            ["gimp", "pl", "sy", "ma", "recolor", "after-recolor",]
        );
        assert_eq!(sk.unknown, ["typo"]);
    }

    #[test]
    fn skip_of_config_toml_is_taken_into_account() {
        let (_d, repo, env) = fixture();
        write(
            &repo,
            "packages.toml",
            "[packages]\ngimp = \"x\"\nvlc = \"y\"\n",
        );
        write(
            &repo,
            "steps.toml",
            "[[manual]]\nid = \"m1\"\ntitle = \"m1\"\n[[manual]]\nid = \"m2\"\ntitle = \"m2\"\n",
        );
        fs::create_dir_all(env.config_file().parent().unwrap()).unwrap();
        fs::write(env.config_file(), "skip = [\"gimp\", \"m1\", \"zzz\"]\n").unwrap();
        let ctx = Ctx::new(&env, &repo);
        let (wanted, steps, sk) = load_wanted(&ctx).unwrap();
        assert_eq!(wanted.keys().collect::<Vec<_>>(), ["vlc"]);
        assert_eq!(manual_pending(&ctx, &steps).len(), 1);
        assert_eq!(sk.ignored, ["gimp", "m1"]);
        assert_eq!(sk.unknown, ["zzz"]);
    }
}
