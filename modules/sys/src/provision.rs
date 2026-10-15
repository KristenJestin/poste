//! Mise en place de la machine décrite par `bootstrap/` : paquets, étapes système (affichées,
//! jamais lancées), étapes utilisateur (faites par poste après confirmation), étapes manuelles.
//!
//! Aucune étape ne lance `sudo`. Les données sont dans `bootstrap/packages.toml` et
//! `bootstrap/steps.toml` ; les contenus de fichiers root dans `bootstrap/system/`.

use anyhow::{bail, Context, Result};
use poste_core::{time, Config, Env};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Variable réservée aux tests : programme (sans argument) lancé à la place de `pacman -Qq`.
pub const TEST_PACMAN_VAR: &str = "POSTE_TEST_PACMAN_QQ";

// ------------------------------------------------------------------ données

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

/// Contexte : chemins et substitutions (`@HOME@`, `@USER@`, `@UID@`, `@GID@`, `@REPO@`,
/// `@BOOTSTRAP@`, et `@DATA@` : le dépôt de données de la machine, vide sans lui).
pub struct Ctx<'a> {
    env: &'a Env,
    dir: PathBuf,
    /// `bootstrap/` du dépôt de données, s'il est déclaré et existe.
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
            std::fs::read_to_string(&p).with_context(|| format!("lecture de {}", p.display()))?;
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
    toml::from_str(&s).with_context(|| format!("lecture de {}", p.display()))
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
        toml::from_str(&s).with_context(|| format!("lecture de {}", p.display()))?;
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

/// Ce que `skip` (config.toml) a retiré du bootstrap de cette machine.
#[derive(Debug, Default, PartialEq)]
pub struct Skipped {
    /// Identifiants retirés (ceux de `skip` qui existent, puis les étapes qui en dépendent).
    pub ignored: Vec<String>,
    /// Identifiants de `skip` qui ne sont ni un paquet ni une étape (faute de frappe ?).
    pub unknown: Vec<String>,
}

/// Retire des paquets et des étapes ce que `skip` désigne. Une étape utilisateur dont un
/// `trigger` est sauté est sautée à son tour (elle suppose cette étape : recolorer un plugin
/// absent échouerait), et ainsi de suite.
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

/// Paquets voulus et étapes, `skip` de la machine appliqué ; avertit des identifiants inconnus.
fn load_wanted(ctx: &Ctx) -> Result<(BTreeMap<String, String>, Steps, Skipped)> {
    let cfg = Config::load(ctx.env)?;
    let mut wanted = load_packages(ctx)?;
    let mut steps = load_steps(ctx)?;
    let sk = apply_skip(&cfg.skip, &mut wanted, &mut steps);
    for u in &sk.unknown {
        eprintln!(
            "avertissement : « {u} » (config.toml, skip) n'est ni un paquet de packages.toml ni l'id d'une étape de steps.toml (faute de frappe ?)"
        );
    }
    Ok((wanted, steps, sk))
}

/// Nature de chaque identifiant que `skip` peut désigner (paquets et étapes du bootstrap).
pub fn skip_catalog(env: &Env, repo: &Path) -> Result<BTreeMap<String, &'static str>> {
    let ctx = Ctx::new(env, repo);
    let steps = load_steps(&ctx)?;
    let mut out: BTreeMap<String, &'static str> = BTreeMap::new();
    for k in load_packages(&ctx)?.into_keys() {
        out.insert(k, "paquet");
    }
    let kinds: [(Vec<&str>, &'static str); 5] = [
        (
            steps.plugin.iter().map(|s| s.id.as_str()).collect(),
            "plugin",
        ),
        (
            steps.file.iter().map(|s| s.id.as_str()).collect(),
            "fichier",
        ),
        (
            steps.user.iter().map(|s| s.id.as_str()).collect(),
            "étape utilisateur",
        ),
        (
            steps.system.iter().map(|s| s.id.as_str()).collect(),
            "étape système",
        ),
        (
            steps.manual.iter().map(|s| s.id.as_str()).collect(),
            "étape manuelle",
        ),
    ];
    for (ids, kind) in kinds {
        for id in ids {
            out.entry(id.to_string()).or_insert(kind);
        }
    }
    Ok(out)
}

/// Effet de `skip` sur ce dépôt, sans avertissement (pour `poste sys skip`).
pub fn skip_effect(env: &Env, repo: &Path, skip: &[String]) -> Result<Skipped> {
    let ctx = Ctx::new(env, repo);
    let mut wanted = load_packages(&ctx)?;
    let mut steps = load_steps(&ctx)?;
    Ok(apply_skip(skip, &mut wanted, &mut steps))
}

fn print_skipped(sk: &Skipped) {
    if !sk.ignored.is_empty() {
        println!(
            "\nignorés sur cette machine (config.toml, skip) : {}",
            sk.ignored.join(", ")
        );
    }
}

// ------------------------------------------------------------------ utilitaires

/// Lance une commande sans sortie ; vrai si elle se termine avec le code 0.
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

/// Paquets installés (`pacman -Qq`, ou le programme de test).
fn installed_packages() -> Result<HashSet<String>> {
    let out = match std::env::var_os(TEST_PACMAN_VAR) {
        Some(p) if !p.is_empty() => Command::new(p).output(),
        _ => Command::new("pacman").arg("-Qq").output(),
    }
    .context("impossible de lancer pacman -Qq")?;
    if !out.status.success() {
        bail!("pacman -Qq a échoué");
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Paquets voulus qui ne sont pas installés (ordre alphabétique).
fn missing_of(wanted: &BTreeMap<String, String>, installed: &HashSet<String>) -> Vec<String> {
    wanted
        .keys()
        .filter(|k| !installed.contains(*k))
        .cloned()
        .collect()
}

// ------------------------------------------------------------------ étapes système

#[derive(Debug, PartialEq)]
enum Sys {
    Done,
    Todo(&'static str),
    /// Non vérifiable sans root : reste affiché tant qu'il n'est pas marqué fait.
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
        return Sys::NotApplicable("sans objet sur cette machine".into());
    }
    let verified = if let (Some(file), Some(src)) = (&s.file, &s.source) {
        let want = match ctx.read_source(src) {
            Ok(w) => w,
            Err(_) => return Sys::Unknown("source illisible"),
        };
        match std::fs::read_to_string(ctx.sub(file)) {
            Ok(c) if c.trim_end() == want.trim_end() => Sys::Done,
            Ok(_) => Sys::Todo("contenu différent"),
            Err(e) if e.kind() == ErrorKind::NotFound => Sys::Todo("fichier absent"),
            Err(_) => Sys::Unknown("illisible sans root"),
        }
    } else if let (Some(file), Some(needle)) = (&s.file, &s.contains) {
        match std::fs::read_to_string(ctx.sub(file)) {
            Ok(c) if c.contains(&ctx.sub(needle)) => Sys::Done,
            Ok(_) => Sys::Todo("ligne absente"),
            Err(e) if e.kind() == ErrorKind::NotFound => Sys::Todo("fichier absent"),
            Err(_) => Sys::Unknown("illisible sans root"),
        }
    } else if !s.check.is_empty() {
        if succeeds(&ctx.sub_cmd(&s.check)) {
            Sys::Done
        } else {
            Sys::Todo("à faire")
        }
    } else {
        Sys::Unknown("non vérifiable")
    };
    match verified {
        Sys::Unknown(_) if is_marked(ctx.env, &s.id) => Sys::Done,
        v => v,
    }
}

/// Les lignes du bloc à copier pour une étape système.
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

// ------------------------------------------------------------------ étapes utilisateur

#[derive(Debug, Clone)]
enum Act {
    Cmd(Vec<String>),
    Write { target: PathBuf, content: String },
}

impl Act {
    fn show(&self) -> String {
        match self {
            Act::Cmd(c) => c.join(" "),
            Act::Write { target, .. } => format!("écrire {}", target.display()),
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
    /// Étapes déclenchées seulement si l'une des étapes citées vient d'être faite.
    triggered: Vec<(Vec<String>, Todo)>,
    /// Ce qui est ignoré, ambigu ou différent : affiché, jamais touché.
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

/// Classe un plugin tiers : rien à faire (`Ok(vec![])`), des actes, ou une note « à la main ».
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
                "{} : absent, et la commande omarchy est introuvable",
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
            "{} : dossier présent mais sans git, rien n'est touché",
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
                "{} : à {} au lieu de {} et fichiers modifiés (patch déjà posé ?) : à traiter à la main",
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
            // déjà appliqué
        } else if git_ok(&dir, &["apply", "--check", pa]) {
            acts.push(cmd(&["git", "-C", &d, "apply", pa]));
        } else {
            return Err(format!(
                "{} : le patch ne s'applique plus (plugin mis à jour ?) : {pa}",
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
                title: format!("plugin {} à {}", p.id, p.commit),
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
                "{} : {} diffère du dépôt, laissé tel quel (modifiez-le ou supprimez-le pour qu'il soit reposé)",
                f.id,
                target.display()
            )),
            Err(e) if e.kind() == ErrorKind::NotFound => plan.todos.push(Todo {
                id: f.id.clone(),
                title: f.title.clone(),
                acts: vec![Act::Write { target, content }],
            }),
            Err(e) => plan.notes.push(format!("{} : {} illisible : {e}", f.id, target.display())),
        }
    }
    for u in &steps.user {
        if let Some(r) = &u.requires {
            if !have(ctx.env, r) {
                plan.notes.push(format!(
                    "{} : ignoré, {r} introuvable (installez le paquet puis relancez)",
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
            let (prog, args) = c.split_first().context("commande vide")?;
            let st = Command::new(prog)
                .args(args)
                .status()
                .with_context(|| format!("impossible de lancer {prog}"))?;
            if !st.success() {
                bail!("{} a échoué ({st})", c.join(" "));
            }
            Ok(())
        }
        Act::Write { target, content } => {
            let parent = target.parent().context("cible sans dossier parent")?;
            std::fs::create_dir_all(parent)?;
            let tmp = parent.join(format!(".poste-tmp-{}", std::process::id()));
            std::fs::write(&tmp, content)?;
            std::fs::rename(&tmp, target)
                .with_context(|| format!("écriture de {}", target.display()))
        }
    }
}

/// Résultat de l'application des étapes utilisateur.
#[derive(Debug, Default)]
pub struct Applied {
    pub done: Vec<String>,
    pub failed: Vec<String>,
}

fn apply_user(plan: &UserPlan) -> Applied {
    let mut res = Applied::default();
    let run_todo = |t: &Todo, res: &mut Applied| {
        println!("\n  {} : {}", t.id, t.title);
        for a in &t.acts {
            println!("    $ {}", a.show());
            if let Err(e) = run_act(a) {
                eprintln!("    échec : {e:#}");
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

/// Les étapes déclenchées qui s'exécuteraient si leurs déclencheurs sont dans le plan.
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
    println!("\nÉtapes manuelles restantes (à faire à la main, puis `poste sys done <id>`)");
    for m in pending {
        println!("  [{}] {}", m.id, ctx.sub(&m.title));
    }
}

/// `poste sys done [<id>] [--undo]` : marque une étape manuelle (ou système non vérifiable) faite.
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
                    // seulement celles que poste ne peut pas vérifier
                    s.check.is_empty() && s.contains.is_none() && s.file.is_some()
                })
                .map(|s| (s.id.as_str(), s.title.as_str())),
        )
        .collect();
    let Some(id) = id else {
        println!("Étapes qui se marquent à la main :");
        for (i, t) in &known {
            let mark = if is_marked(env, i) {
                "fait"
            } else {
                "à faire"
            };
            println!("  [{i}] ({mark}) {}", ctx.sub(t));
        }
        return Ok(());
    };
    if !known.iter().any(|(i, _)| *i == id) {
        bail!(
            "étape inconnue : {id}. Connues : {}",
            known.iter().map(|(i, _)| *i).collect::<Vec<_>>().join(", ")
        );
    }
    let f = done_dir(env).join(id);
    if undo {
        match std::fs::remove_file(&f) {
            Ok(()) => println!("{id} : de nouveau à faire"),
            Err(e) if e.kind() == ErrorKind::NotFound => println!("{id} : n'était pas marqué fait"),
            Err(e) => return Err(e.into()),
        }
    } else {
        std::fs::create_dir_all(done_dir(env))?;
        std::fs::write(&f, format!("{}\n", time::now_rfc3339()))?;
        println!("{id} : marqué fait");
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
        Err(e) => println!("  vérification impossible : {e:#}"),
        Ok(None) => println!("  aucun paquet déclaré"),
        Ok(Some(p)) if p.missing.is_empty() => println!("  tous présents ({})", p.total),
        Ok(Some(p)) => {
            println!(
                "  manquants ({}/{}) : {}",
                p.missing.len(),
                p.total,
                p.missing.join(" ")
            );
            println!("  À lancer vous-même (poste ne l'exécute pas) :");
            println!("    yay -S --needed {}", p.missing.join(" "));
        }
    }
}

/// Affiche les étapes système en attente ; renvoie leur nombre.
fn print_system(ctx: &Ctx, steps: &Steps) -> Result<usize> {
    let mut block: Vec<String> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for s in &steps.system {
        match sys_state(ctx, s) {
            Sys::Done | Sys::NotApplicable(_) => {}
            st => {
                let (tag, why) = match st {
                    Sys::Todo(w) => ("à faire", w),
                    Sys::Unknown(w) => ("à vérifier", w),
                    _ => unreachable!(),
                };
                lines.push(format!(
                    "  [{}] {} ({tag} : {why})",
                    s.id,
                    ctx.sub(&s.title)
                ));
                if tag == "à vérifier" {
                    lines.push(format!(
                        "      si c'est déjà fait : poste sys done {}",
                        s.id
                    ));
                }
                block.push(format!("# {} : {}", s.id, ctx.sub(&s.title)));
                block.extend(sys_commands(ctx, s)?);
                block.push(String::new());
            }
        }
    }
    println!("\nÉtapes système (sudo : à lancer vous-même, poste ne le fait jamais)");
    if lines.is_empty() {
        println!("  rien à faire");
        return Ok(0);
    }
    for l in &lines {
        println!("{l}");
    }
    println!("\n  Bloc à copier :");
    println!("  ----------------------------------------");
    for l in &block {
        println!("{l}");
    }
    println!("  ----------------------------------------");
    Ok(lines.iter().filter(|l| l.starts_with("  [")).count())
}

fn print_user_plan(plan: &UserPlan) {
    println!("\nÉtapes utilisateur (faites par poste)");
    if plan.todos.is_empty() {
        println!("  rien à faire");
    }
    for t in &plan.todos {
        println!("  à faire  {} : {}", t.id, t.title);
        for a in &t.acts {
            println!("             $ {}", a.show());
        }
    }
    for t in triggered_in_plan(plan) {
        println!("  ensuite  {} : {}", t.id, t.title);
        for a in &t.acts {
            println!("             $ {}", a.show());
        }
    }
    for n in &plan.notes {
        println!("  note     {n}");
    }
}

// ------------------------------------------------------------------ points d'entrée

/// Phase « bootstrap » de `setup` : paquets, système, utilisateur (avec confirmation), manuel.
/// Renvoie les étapes utilisateur appliquées et si une a échoué.
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
            println!("\nSimulation (--dry-run) : étapes utilisateur non lancées");
        } else if yes || confirm(&format!("\nLancer ces {n} étape(s) utilisateur ? [o/N] "))? {
            applied = apply_user(&plan);
        } else {
            println!("Étapes utilisateur non lancées.");
        }
    }
    print_manual(&ctx, &steps);
    print_skipped(&skipped);
    Ok(applied)
}

/// Lignes de `poste sys status` pour paquets, étapes système, utilisateur et manuelles.
pub fn status(env: &Env, repo: &Path) -> Result<()> {
    let ctx = Ctx::new(env, repo);
    let (wanted, steps, skipped) = load_wanted(&ctx)?;
    println!("\nPaquets");
    match packages_report(&wanted) {
        Err(e) => println!("  vérification impossible : {e:#}"),
        Ok(None) => println!("  aucun paquet déclaré"),
        Ok(Some(p)) if p.missing.is_empty() => println!("  tous présents ({})", p.total),
        Ok(Some(p)) => {
            println!("  manquants ({}/{}) :", p.missing.len(), p.total);
            for m in &p.missing {
                println!("    {m}");
            }
        }
    }
    println!("\nÉtapes système");
    let mut any = false;
    for s in &steps.system {
        match sys_state(&ctx, s) {
            Sys::Todo(w) => {
                any = true;
                println!("  à faire      [{}] {} ({w})", s.id, ctx.sub(&s.title));
            }
            Sys::Unknown(w) => {
                any = true;
                println!("  à vérifier   [{}] {} ({w})", s.id, ctx.sub(&s.title));
            }
            _ => {}
        }
    }
    if !any {
        println!("  rien en attente");
    }
    let plan = user_plan(&ctx, &steps)?;
    println!("\nÉtapes utilisateur");
    if plan.todos.is_empty() && plan.notes.is_empty() {
        println!("  rien en attente");
    }
    for t in &plan.todos {
        println!("  à faire  [{}] {}", t.id, t.title);
    }
    for n in &plan.notes {
        println!("  note     {n}");
    }
    let manual = manual_pending(&ctx, &steps);
    println!("\nÉtapes manuelles");
    if manual.is_empty() {
        println!("  toutes faites");
    }
    for m in manual {
        println!("  à faire  [{}] {}", m.id, ctx.sub(&m.title));
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
    fn paquets_manquants() {
        let wanted: BTreeMap<String, String> = [("a", ""), ("b", ""), ("c", "")]
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let have: HashSet<String> = ["a", "c", "z"].iter().map(|s| s.to_string()).collect();
        assert_eq!(missing_of(&wanted, &have), vec!["b"]);
    }

    #[test]
    fn etapes_et_paquets_du_depot_de_donnees() {
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
        dw("system/x.line", "LIGNE @DATA@\n");
        dw(
            "steps.toml",
            "[[system]]\nid = \"s\"\ntitle = \"vrai\"\nfile = \"/f\"\nline = \"system/x.line\"\n\
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
        // Même id : l'étape du dépôt de données remplace celle du dépôt.
        assert_eq!(steps.system.len(), 1);
        assert_eq!(steps.system[0].title, "vrai");
        let ids: Vec<_> = steps.manual.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["m", "n"]);
        // Source lue dans le bootstrap/ du dépôt de données, @DATA@ remplacé.
        let cmds = sys_commands(&ctx, &steps.system[0]).unwrap();
        assert!(
            cmds[0].contains(&format!("LIGNE {}", data.display())),
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
    fn etat_systeme_fichier_ligne_et_marqueur() {
        let (_d, repo, env) = fixture();
        write(&repo, "system/r.rules", "règle pour @HOME@\n");
        write(&repo, "system/l.line", "LIGNE-X\n");
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
contains = "LIGNE-X"
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
        assert_eq!(st(0), Sys::Todo("fichier absent"));
        fs::write(&target, "autre").unwrap();
        assert_eq!(st(0), Sys::Todo("contenu différent"));
        fs::write(&target, format!("règle pour {}\n", env.home.display())).unwrap();
        assert_eq!(st(0), Sys::Done);
        fs::write(&fstab, "x\n").unwrap();
        assert_eq!(st(1), Sys::Todo("ligne absente"));
        fs::write(&fstab, "x\nLIGNE-X\n").unwrap();
        assert_eq!(st(1), Sys::Done);
        assert_eq!(st(2), Sys::Todo("à faire"));
        assert!(matches!(st(3), Sys::NotApplicable(_)));
        let cmds = sys_commands(&ctx, &steps.system[0]).unwrap();
        assert!(cmds[0].starts_with("sudo tee ") && cmds[0].contains("<<'EOF'"));
        assert_eq!(
            sys_commands(&ctx, &steps.system[1]).unwrap()[1],
            "sudo true"
        );
        // Illisible : inconnu, puis fait une fois marqué.
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
    fn marquer_fait_et_annuler() {
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
            "m2 vérifiée par sa commande"
        );
        assert!(done(&env, &repo, Some("zzz"), false).is_err());
        done(&env, &repo, Some("m1"), false).unwrap();
        assert!(manual_pending(&ctx, &steps).is_empty());
        done(&env, &repo, Some("m1"), true).unwrap();
        assert_eq!(manual_pending(&ctx, &steps).len(), 1);
    }

    #[test]
    fn plugin_commit_patch_et_fichier() {
        let (_d, repo, env) = fixture();
        let ctx = Ctx::new(&env, &repo);
        let dir = ctx.plugin_dir("p");
        fs::create_dir_all(&dir).unwrap();
        sh(&dir, &["init", "-q"]);
        fs::write(dir.join("f.txt"), "un\n").unwrap();
        sh(&dir, &["add", "."]);
        sh(&dir, &["commit", "-q", "-m", "1"]);
        let head = git(&dir, &["rev-parse", "--short", "HEAD"]).unwrap();
        // Patch : change « un » en « deux ».
        write(
            &repo,
            "patches/p.patch",
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1 +1 @@\n-un\n+deux\n",
        );
        let plug = |commit: &str| Plugin {
            id: "p".into(),
            url: "u".into(),
            commit: commit.into(),
            patch: Some("patches/p.patch".into()),
        };
        // Bon commit, patch à poser.
        let acts = plugin_acts(&ctx, &plug(&head)).unwrap();
        assert_eq!(acts.len(), 1);
        run_act(&acts[0]).unwrap();
        assert_eq!(fs::read_to_string(dir.join("f.txt")).unwrap(), "deux\n");
        // Déjà posé : rien.
        assert!(plugin_acts(&ctx, &plug(&head)).unwrap().is_empty());
        // Autre commit voulu et arbre modifié : note, rien à faire.
        assert!(plugin_acts(&ctx, &plug("deadbee")).is_err());
        // Patch qui ne s'applique plus.
        fs::write(dir.join("f.txt"), "trois\n").unwrap();
        assert!(plugin_acts(&ctx, &plug(&head)).is_err());
        // Plugin absent sans omarchy : note.
        let mut p = plug(&head);
        p.id = "absent".into();
        assert_eq!(
            plugin_acts(&ctx, &p).is_ok(),
            have(&env, "omarchy"),
            "absent : omarchy plugin add si omarchy existe, sinon note"
        );

        // Fichier JSON : absent -> écrit ; même JSON mis en forme autrement -> rien ; différent -> note.
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
    fn etapes_utilisateur_check_et_declencheur() {
        let (_d, repo, env) = fixture();
        let ctx = Ctx::new(&env, &repo);
        let marker = env.home.join("marqueur");
        let steps: Steps = toml::from_str(&format!(
            r#"
[[user]]
id = "fait"
title = "déjà fait"
check = ["true"]
run = [["false"]]
[[user]]
id = "a"
title = "a"
check = ["false"]
run = [["touch", "{m}"]]
[[user]]
id = "apres"
title = "après a"
trigger = ["a"]
run = [["true"]]
[[user]]
id = "sans-bin"
title = "x"
requires = "binaire-qui-n-existe-pas"
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
        assert_eq!(r.done, vec!["a", "apres"]);
        assert!(r.failed.is_empty() && marker.is_file());
    }

    #[test]
    fn skip_retire_paquets_etapes_et_dependantes() {
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
id = "apres-recolor"
title = "t"
trigger = ["recolor"]
[[user]]
id = "autre"
title = "t"
trigger = ["cfg"]
[[user]]
id = "libre"
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
        let skip: Vec<String> = ["gimp", "pl", "sy", "ma", "gimp", "faute", "faute"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let sk = apply_skip(&skip, &mut wanted, &mut steps);
        assert_eq!(wanted.keys().collect::<Vec<_>>(), ["vlc"]);
        assert!(steps.plugin.is_empty() && steps.system.is_empty() && steps.manual.is_empty());
        let users: Vec<&str> = steps.user.iter().map(|u| u.id.as_str()).collect();
        assert_eq!(users, ["autre", "libre"], "autre dépend de cfg, non sauté");
        assert_eq!(steps.file.len(), 1, "cfg n'est pas sauté");
        assert_eq!(
            sk.ignored,
            ["gimp", "pl", "sy", "ma", "recolor", "apres-recolor",]
        );
        assert_eq!(sk.unknown, ["faute"]);
    }

    #[test]
    fn skip_de_config_toml_pris_en_compte() {
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
