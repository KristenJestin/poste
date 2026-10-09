//! La config de l'utilisateur : `config/` du dépôt reflète `~` (vrais noms, sans préfixes)
//! et est posée par liens symboliques. Ce module classe l'état de chaque cible, prépare le
//! plan de `poste sys setup` et l'applique, sans jamais écraser sans sauvegarde.
//!
//! Dans `config/` :
//! - un fichier ordinaire devient, dans `~`, un lien vers ce fichier du dépôt ;
//! - un lien symbolique du dépôt est recréé tel quel dans `~` (même cible) ;
//! - un dossier listé dans `config/.poste/link-dirs` est lié d'un bloc ;
//! - `config/.poste/modes` donne les droits restreints (`chemin mode`), réappliqués à la
//!   cible réelle du lien ; `config/.poste/ignore` liste ce que `add` refuse.

use crate::manifest::{Entry as ManifestEntry, Manifest};
use crate::util::{backup, copy_tree, replace_with_symlink, sha256_file};
use anyhow::{bail, Context, Result};
use poste_core::{time, Env};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

/// Nom du module dans `installed.json`.
pub const MODULE: &str = "config";
/// Dossier des métadonnées, à la racine de `config/`.
pub const META: &str = ".poste";
const PYCACHE: &str = "__pycache__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Fichier ordinaire : lien vers le fichier du dépôt.
    File,
    /// Lien symbolique du dépôt : recréé avec la même cible.
    Link(PathBuf),
    /// Dossier lié d'un bloc (`link-dirs`).
    Dir,
}

#[derive(Debug, Clone)]
pub struct Entry {
    /// Chemin relatif à `~`, séparé par `/`.
    pub rel: String,
    /// Dans le dépôt : `config/<rel>`.
    pub src: PathBuf,
    /// Dans `~`.
    pub dest: PathBuf,
    pub kind: Kind,
}

#[derive(Debug)]
pub struct Layout {
    /// `<dépôt>/config`.
    pub root: PathBuf,
    pub entries: Vec<Entry>,
    pub modes: BTreeMap<String, u32>,
    pub link_dirs: Vec<String>,
}

// ---------------------------------------------------------------- lecture du dépôt

/// Normalise un chemin relatif à `~` : sans `./`, `~/`, ni `/` final ; refuse `..`.
pub fn normalize_rel(s: &str) -> Result<String> {
    let s = s.trim();
    let s = s.strip_prefix("~/").unwrap_or(s);
    let s = s.strip_prefix("./").unwrap_or(s);
    let s = s.trim_end_matches('/');
    if s.is_empty() || s.starts_with('/') {
        bail!("chemin invalide : « {s} » (attendu : relatif à ~)");
    }
    if s.split('/').any(|c| c == ".." || c == "." || c.is_empty()) {
        bail!("chemin invalide : « {s} »");
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
            .with_context(|| format!("modes, ligne {n} : attendu « chemin mode »"))?;
        let m = u32::from_str_radix(mode.trim(), 8)
            .ok()
            .filter(|m| *m <= 0o777)
            .with_context(|| format!("modes, ligne {n} : mode octal invalide « {mode} »"))?;
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
        Err(e) => Err(e).with_context(|| format!("lecture de {}", f.display())),
    }
}

/// Lit `config/` ; `None` si le dépôt n'a pas de dossier `config/`.
pub fn load(repo: &Path, env: &Env) -> Result<Option<Layout>> {
    let root = repo.join("config");
    if !root.is_dir() {
        return Ok(None);
    }
    let modes = parse_modes(&read_meta(&root, "modes")?)?;
    let link_dirs = parse_link_dirs(&read_meta(&root, "link-dirs")?)?;
    for d in &link_dirs {
        let md = std::fs::symlink_metadata(root.join(d));
        if !matches!(&md, Ok(m) if m.is_dir()) {
            bail!("link-dirs : « {d} » n'est pas un dossier de config/");
        }
        if let Some(o) = link_dirs
            .iter()
            .find(|o| *o != d && d.starts_with(&format!("{o}/")))
        {
            bail!("link-dirs : « {d} » est dans « {o} », déjà lié d'un bloc");
        }
    }
    let mut entries = Vec::new();
    scan("", &link_dirs, env, &root, &mut entries)?;
    Ok(Some(Layout {
        root,
        entries,
        modes,
        link_dirs,
    }))
}

fn scan(
    prefix: &str,
    link_dirs: &[String],
    env: &Env,
    dir: &Path,
    out: &mut Vec<Entry>,
) -> Result<()> {
    let mut children: Vec<_> = std::fs::read_dir(dir)
        .with_context(|| format!("lecture de {}", dir.display()))?
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

// ---------------------------------------------------------------- états

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Déjà lié comme prévu.
    Linked,
    /// Rien à la cible.
    Missing,
    /// Fichier (ou dossier) ordinaire identique au dépôt.
    Identical,
    /// Fichier (ou dossier) ordinaire différent du dépôt.
    Differs,
    /// Lien qui pointe ailleurs (`broken` : la cible n'existe pas).
    OtherLink { target: PathBuf, broken: bool },
    /// Mauvais type (dossier à la place d'un fichier, etc.).
    WrongType(&'static str),
    /// Un dossier parent n'est pas un dossier.
    ParentBlocked(PathBuf),
}

#[derive(Debug, PartialEq, Eq)]
enum Node {
    File(String),
    Link(PathBuf),
}

/// Contenu d'un arbre (sans `__pycache__`, sans dossiers vides), pour comparer deux dossiers.
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

fn same_target(a: &Path, b: &Path) -> bool {
    a == b
        || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

pub fn state_of(e: &Entry, env: &Env) -> Result<State> {
    let md = match std::fs::symlink_metadata(&e.dest) {
        Ok(m) => m,
        Err(err)
            if err.kind() == std::io::ErrorKind::NotFound
                || err.raw_os_error() == Some(20) /* ENOTDIR */ =>
        {
            // Un parent existant doit être un dossier (ou un lien vers un dossier).
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
        Err(err) => return Err(err).with_context(|| format!("lecture de {}", e.dest.display())),
    };
    if md.file_type().is_symlink() {
        let target = std::fs::read_link(&e.dest)?;
        let ok = match &e.kind {
            Kind::Link(t) => *t == target,
            Kind::File | Kind::Dir => same_target(&target, &e.src),
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
        (Kind::File, true, _) => State::WrongType("dossier à la place d'un fichier"),
        (Kind::Link(_), true, _) => State::WrongType("dossier à la place d'un lien"),
        (Kind::Link(_), _, true) => State::WrongType("fichier ordinaire à la place d'un lien"),
        (Kind::Dir, _, true) => State::WrongType("fichier à la place d'un dossier"),
        _ => State::WrongType("type de fichier inattendu"),
    })
}

// ---------------------------------------------------------------- modes

#[derive(Debug, Clone)]
pub struct ModeFix {
    pub rel: String,
    /// Chemin réel à modifier (dépôt pour un fichier lié, `~` pour un simple dossier).
    pub path: PathBuf,
    pub from: u32,
    pub to: u32,
}

/// Droits à corriger, calculés sur la cible réelle du lien.
pub fn mode_fixes(layout: &Layout, env: &Env) -> Vec<ModeFix> {
    let mut out = Vec::new();
    for (rel, want) in &layout.modes {
        let in_repo = layout.root.join(rel);
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
        // Un dossier ordinaire (non lié) : ses droits sont ceux du dossier de `~`.
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
    /// Rien à faire (déjà lié).
    Done,
    /// Cible absente : créer le lien.
    Create,
    /// Ordinaire identique : sauvegarde puis lien.
    Replace,
    /// Décision `--take-repo` : sauvegarde de la cible, puis lien.
    TakeRepo,
    /// Décision `--take-home` : la cible réelle est recopiée dans le dépôt, puis lien.
    TakeHome,
    /// Conflit, non traité.
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
    /// Nombre d'écritures prévues.
    pub fn writes(&self) -> usize {
        self.steps
            .iter()
            .filter(|s| !matches!(s.op, Op::Done | Op::Conflict))
            .count()
            + self.modes.len()
    }
}

/// Vérifie qu'un `--take-home` est réalisable pour cet état.
fn check_take_home(e: &Entry, st: &State) -> Result<()> {
    let ok = match (&e.kind, st) {
        (Kind::File, State::Differs) => true,
        (Kind::File, State::OtherLink { broken, .. }) => {
            if *broken {
                bail!(
                    "--take-home {} : le lien est cassé, rien à reprendre",
                    e.rel
                )
            }
            std::fs::metadata(&e.dest)
                .map(|m| m.is_file())
                .unwrap_or(false)
        }
        (Kind::Dir, State::Differs) => true,
        (Kind::Link(_), State::OtherLink { .. }) => true,
        _ => false,
    };
    if !ok {
        bail!(
            "--take-home {} : impossible dans l'état actuel de la cible ({}) ; utilisez --take-repo",
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
            bail!("« {k} » est dans le dossier lié d'un bloc « {d} » : tranchez sur le dossier");
        }
        bail!("« {k} » n'est pas une entrée de config/");
    }
    let mut p = Plan::default();
    for e in &layout.entries {
        let state = state_of(e, env)?;
        let decision = decisions.get(&e.rel).copied();
        let op = match (&state, decision) {
            (State::Linked, d) => {
                if d.is_some() {
                    p.warnings
                        .push(format!("{} : déjà lié, rien à trancher", e.rel));
                }
                Op::Done
            }
            (State::Missing, d) => {
                if d.is_some() {
                    p.warnings
                        .push(format!("{} : cible absente, rien à trancher", e.rel));
                }
                Op::Create
            }
            (State::Identical, d) => {
                if d.is_some() {
                    p.warnings
                        .push(format!("{} : identique au dépôt, rien à trancher", e.rel));
                }
                Op::Replace
            }
            (State::ParentBlocked(_), Some(_)) => bail!(
                "{} : un dossier parent n'est pas un dossier, impossible de trancher",
                e.rel
            ),
            (_, Some(Decision::Repo)) => Op::TakeRepo,
            (st, Some(Decision::Home)) => {
                check_take_home(e, st)?;
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
        State::Linked => "lié".into(),
        State::Missing => "absent".into(),
        State::Identical => "fichier ordinaire identique".into(),
        State::Differs => "différent du dépôt".into(),
        State::OtherLink { target, broken } => format!(
            "lien vers {}{}",
            target.display(),
            if *broken { " (cassé)" } else { "" }
        ),
        State::WrongType(s) => (*s).to_string(),
        State::ParentBlocked(p) => format!("{} n'est pas un dossier", p.display()),
    }
}

// ---------------------------------------------------------------- affichage

pub fn print_plan(env: &Env, plan: &Plan) {
    println!("\nConfig (~ <- config/ par liens) :");
    let linked = plan.count(&Op::Done);
    for s in &plan.steps {
        let d = env.display(&s.entry.dest);
        let kind = if s.entry.kind == Kind::Dir {
            " (dossier)"
        } else {
            ""
        };
        match s.op {
            Op::Done | Op::Conflict => {}
            Op::Create => println!("  lien créé   {d}{kind}"),
            Op::Replace => {
                println!("  remplacé    {d}{kind}  (identique au dépôt, ancien sauvegardé)")
            }
            Op::TakeRepo => println!(
                "  remplacé    {d}{kind}  (--take-repo : version du dépôt, ancien sauvegardé)"
            ),
            Op::TakeHome => println!(
                "  repris      {d}{kind}  (--take-home : copié dans le dépôt, ancien sauvegardé)"
            ),
        }
    }
    for m in &plan.modes {
        println!("  droits      {}  {:04o} -> {:04o}", m.rel, m.from, m.to);
    }
    for w in &plan.warnings {
        println!("  note        {w}");
    }
    println!(
        "  bilan : {linked} déjà lié(s), {} à créer, {} à remplacer (identiques), {} décision(s) appliquée(s), {} droit(s) à corriger, {} conflit(s)",
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
        "\nConflits ({}) : rien n'a été touché pour ces entrées.",
        c.len()
    );
    for s in c {
        println!("  {}  [{}]", env.display(&s.entry.dest), describe(&s.state));
        let rel = &s.entry.rel;
        match s.state {
            State::Differs => println!(
                "      voir : poste sys diff {rel}\n      trancher : poste sys setup --take-repo {rel}  ou  --take-home {rel}"
            ),
            State::ParentBlocked(_) => {
                println!("      à régler à la main : un dossier parent est un fichier")
            }
            _ => println!(
                "      trancher : poste sys setup --take-repo {rel}  ou  --take-home {rel}"
            ),
        }
    }
}

// ---------------------------------------------------------------- application

fn link_target(e: &Entry) -> PathBuf {
    match &e.kind {
        Kind::Link(t) => t.clone(),
        _ => e.src.clone(),
    }
}

/// Recopie ce qui est dans `~` (fichier, lien ou dossier) vers le dépôt.
fn take_home(layout: &Layout, e: &Entry, from: &Path) -> Result<()> {
    match &e.kind {
        Kind::File => {
            let md = std::fs::metadata(from)?;
            let tmp = e
                .src
                .with_file_name(format!(".poste-tmp-{}-repris", std::process::id()));
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
    }
    Ok(())
}

/// Copie `src` sur `dst` (sans rien retirer de `dst`), hors `__pycache__`.
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

/// Un fichier repris avec des droits restreints : ajoute son mode à `config/.poste/modes`.
fn note_restricted_mode(layout: &Layout, e: &Entry, mode: u32) -> Result<()> {
    if mode & 0o077 == 0 && !layout.modes.contains_key(&e.rel) {
        upsert_mode_line(&layout.root, &e.rel, mode)?;
        std::fs::set_permissions(&e.src, std::fs::Permissions::from_mode(mode))?;
        println!(
            "  droits {:04o} notés dans config/.poste/modes pour {}",
            mode, e.rel
        );
    }
    Ok(())
}

/// Ajoute ou remplace la ligne d'un chemin dans `config/.poste/modes` (garde les commentaires).
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

/// Empreinte à noter dans `installed.json` : contenu d'un fichier, sinon la cible du lien.
fn record_hash(e: &Entry) -> Result<String> {
    Ok(match e.kind {
        Kind::File => sha256_file(&e.src)?,
        _ => format!(
            "{:x}",
            Sha256::digest(link_target(e).to_string_lossy().as_bytes())
        ),
    })
}

/// Note l'entrée dans le manifeste ; `true` si quelque chose a changé.
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

/// Applique le plan. Renvoie le nombre d'entrées écrites. Met à jour `manifest` (à
/// enregistrer par l'appelant, qui sait aussi enregistrer les entrées déjà liées).
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
            Op::Create => {
                if let Some(parent) = e.dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                symlink(link_target(e), &e.dest)
                    .with_context(|| format!("lien {}", e.dest.display()))?;
            }
            Op::Replace | Op::TakeRepo | Op::TakeHome => {
                if s.op == Op::TakeHome {
                    // Un dossier est déplacé par la sauvegarde : on le recopie depuis là.
                    // Pour le reste, on recopie d'abord, puis on sauvegarde l'original.
                    let bak = if e.kind == Kind::Dir {
                        let bak = backup(env, &e.dest, stamp)?;
                        take_home(layout, e, &bak)?;
                        bak
                    } else {
                        take_home(layout, e, &e.dest)?;
                        backup(env, &e.dest, stamp)?
                    };
                    println!("  sauvegarde : {}", env.display(&bak));
                    if matches!(e.kind, Kind::Link(_)) && state_of(e, env)? == State::Linked {
                        record(manifest, e)?;
                        manifest.save(manifest_file)?;
                        n += 1;
                        continue;
                    }
                } else {
                    let bak = backup(env, &e.dest, stamp)?;
                    println!("  sauvegarde : {}", env.display(&bak));
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
            .with_context(|| format!("droits de {}", m.path.display()))?;
        println!("  droits {:04o} : {}", m.to, m.rel);
        n += 1;
    }
    Ok(n)
}

/// Pose le lien final (la cible a été sauvegardée ; un dossier a déjà été déplacé).
fn place_link(e: &Entry) -> Result<()> {
    let target = link_target(e);
    match std::fs::symlink_metadata(&e.dest) {
        Ok(m) if m.is_dir() => bail!("{} est encore un dossier", e.dest.display()),
        Ok(_) => replace_with_symlink(&e.dest, &target),
        Err(_) => {
            if let Some(parent) = e.dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            symlink(&target, &e.dest).with_context(|| format!("lien {}", e.dest.display()))
        }
    }
}

/// Enregistre dans le manifeste toutes les entrées déjà liées (idempotent) ; `true` si changé.
pub fn record_linked(plan: &Plan, manifest: &mut Manifest) -> Result<bool> {
    let mut changed = false;
    for s in &plan.steps {
        if s.op == Op::Done {
            changed |= record(manifest, &s.entry)?;
        }
    }
    Ok(changed)
}

// ---------------------------------------------------------------- chemins donnés par l'utilisateur

/// Chemin relatif à `~` pour un argument : `~/x`, absolu sous `~`, relatif au dossier
/// courant s'il existe sous `~`, sinon relatif à `~`. Ne suit pas les liens.
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
        .map_err(|_| anyhow::anyhow!("{} n'est pas sous le dossier personnel", p.display()))?
        .components()
    {
        match c {
            Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            Component::CurDir => {}
            _ => bail!("chemin invalide : {arg}"),
        }
    }
    if parts.is_empty() {
        bail!("chemin invalide : {arg}");
    }
    Ok(parts.join("/"))
}

fn entry_for<'a>(layout: &'a Layout, rel: &str) -> Result<&'a Entry> {
    if let Some(e) = layout.entries.iter().find(|e| e.rel == rel) {
        return Ok(e);
    }
    if let Some(d) = layout
        .link_dirs
        .iter()
        .find(|d| rel.starts_with(&format!("{d}/")))
    {
        bail!("{rel} est dans le dossier lié d'un bloc « {d} » : utilisez ce dossier");
    }
    bail!("{rel} n'est pas géré par config/")
}

// ---------------------------------------------------------------- diff

/// `poste sys diff [<chemin>]` : retourne `true` s'il y a des différences.
pub fn diff(layout: &Layout, env: &Env, arg: Option<&str>) -> Result<bool> {
    let entries: Vec<&Entry> = match arg {
        Some(a) => vec![entry_for(layout, &rel_of(env, a)?)?],
        None => layout.entries.iter().collect(),
    };
    let mut any = false;
    let mut others: Vec<(String, String)> = Vec::new();
    for e in entries {
        let st = state_of(e, env)?;
        match st {
            State::Linked | State::Identical => {
                if arg.is_some() {
                    println!("{} : {}, aucune différence.", e.rel, describe(&st));
                }
            }
            State::Missing => {
                if arg.is_some() {
                    println!("{} : absent de ~ (setup créera le lien).", e.rel);
                }
            }
            State::Differs => {
                any = true;
                println!("=== {} (dépôt : -, ~ : +)", e.rel);
                let _ = std::io::Write::flush(&mut std::io::stdout());
                let mut cmd = std::process::Command::new("diff");
                cmd.args(["-ru", "-x", PYCACHE, "--label", &format!("depot/{}", e.rel)]);
                cmd.arg("--label").arg(format!("maison/{}", e.rel));
                cmd.arg(&e.src).arg(&e.dest);
                let status = cmd.status().context("impossible de lancer diff")?;
                if status.code().is_some_and(|c| c > 1) {
                    bail!("diff a échoué pour {}", e.rel);
                }
            }
            other => {
                any = true;
                if arg.is_some() {
                    println!("{} : {}", e.rel, describe(&other));
                    if let (Kind::Link(t), State::OtherLink { target, .. }) = (&e.kind, &other) {
                        println!(
                            "  dépôt : lien vers {}\n  ~     : lien vers {}",
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
        println!("\nAutres conflits (pas de diff possible) :");
        for (p, d) in others {
            println!("  {p}  [{d}]");
        }
    }
    if !any && arg.is_none() {
        println!("Aucune différence : tout ce qui est en place est lié ou identique.");
    }
    Ok(any)
}

// ---------------------------------------------------------------- add / forget

/// Droits à noter pour un fichier ajouté : restreints seulement si aucun droit de groupe/autres.
fn restricted(mode: u32) -> Option<u32> {
    (mode & 0o077 == 0).then_some(mode & 0o777)
}

/// Chemins (fichiers et liens) sous `p` pour `add`, hors `__pycache__`.
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
            "refus : {rel} correspond à la règle « {rule} » (config/.poste/ignore ou règles intégrées : secrets, caches, fichiers régénérés)"
        );
    }
    let home_path = env.home.join(&rel);
    if std::fs::symlink_metadata(&home_path).is_err() {
        bail!("{} n'existe pas", env.display(&home_path));
    }
    let mut files = Vec::new();
    collect_files(&home_path, &mut files)?;
    let mut manifest = Manifest::load(manifest_file)?;
    let stamp = time::now_compact();
    let (mut added, mut skipped) = (0, 0);
    for f in files {
        let frel = f.strip_prefix(&env.home)?.to_string_lossy().into_owned();
        if let Some(rule) = ignore.matching(&frel) {
            println!("  ignoré : {frel} (règle « {rule} »)");
            skipped += 1;
            continue;
        }
        let dest_repo = root.join(&frel);
        let md = std::fs::symlink_metadata(&f)?;
        // Déjà dans le dépôt (lien vers config/, ou sous un dossier lié) ?
        if let (Ok(c), Ok(r)) = (std::fs::canonicalize(&f), std::fs::canonicalize(&root)) {
            if c.starts_with(&r) {
                println!("  déjà géré : {frel}");
                skipped += 1;
                continue;
            }
        }
        if std::fs::symlink_metadata(&dest_repo).is_ok() {
            println!(
                "  existe déjà dans le dépôt : {frel} (tranchez avec poste sys setup --take-home/--take-repo)"
            );
            skipped += 1;
            continue;
        }
        if dry_run {
            println!("  ajouterait : {frel}");
            added += 1;
            continue;
        }
        if let Some(parent) = dest_repo.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if md.file_type().is_symlink() {
            symlink(std::fs::read_link(&f)?, &dest_repo)?;
            println!("  ajouté (lien) : {frel}");
        } else {
            let bak = backup(env, &f, &stamp)?;
            std::fs::copy(&f, &dest_repo)?;
            if sha256_file(&f)? != sha256_file(&dest_repo)? {
                bail!("copie de {frel} différente de l'original, abandon (original intact)");
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
            println!("  ajouté : {frel} (sauvegarde : {})", env.display(&bak));
        }
        let e = Entry {
            rel: frel.clone(),
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
        "{added} ajouté(s){} ; à vérifier puis commiter dans le dépôt.",
        if skipped > 0 {
            format!(", {skipped} ignoré(s)")
        } else {
            String::new()
        }
    );
    if dry_run {
        println!("Simulation (--dry-run) : rien n'a été écrit.");
    }
    Ok(())
}

pub fn forget(layout: &Layout, env: &Env, arg: &str, manifest_file: &Path) -> Result<()> {
    let rel = rel_of(env, arg)?;
    let e = entry_for(layout, &rel)?;
    let st = state_of(e, env)?;
    if st != State::Linked {
        bail!("{rel} n'est pas lié ({}) : rien à retirer", describe(&st));
    }
    match &e.kind {
        Kind::File => {
            let tmp = e
                .dest
                .with_file_name(format!(".poste-tmp-{}-oubli", std::process::id()));
            std::fs::copy(&e.src, &tmp)?;
            std::fs::rename(&tmp, &e.dest)?;
        }
        Kind::Dir => {
            let tmp = e
                .dest
                .with_file_name(format!(".poste-tmp-{}-oubli", std::process::id()));
            copy_tree(&e.src, &tmp)?;
            std::fs::remove_file(&e.dest)?;
            std::fs::rename(&tmp, &e.dest)?;
        }
        Kind::Link(_) => {}
    }
    let mut manifest = Manifest::load(manifest_file)?;
    let p = e.dest.to_string_lossy().into_owned();
    if manifest.files.iter().any(|m| m.path == p) {
        manifest.files.retain(|m| m.path != p);
        manifest.save(manifest_file)?;
    }
    println!(
        "{} : le lien est remplacé par un fichier ordinaire (copie du dépôt).",
        env.display(&e.dest)
    );
    println!(
        "Le fichier reste dans le dépôt jusqu'à votre commit : retirez-le avec « git rm -r config/{rel} » (et sa ligne dans config/.poste/modes ou link-dirs si besoin), sinon le prochain setup le relierait."
    );
    Ok(())
}

// ---------------------------------------------------------------- status

pub fn print_status(layout: &Layout, env: &Env, manifest: &Manifest) -> Result<()> {
    println!("\nConfig ({} entrée(s) dans config/)", layout.entries.len());
    let plan = plan(layout, env, &HashMap::new())?;
    let mut linked = 0;
    let mut lines: Vec<String> = Vec::new();
    let (mut missing, mut broken, mut replaced, mut conflicts) = (0, 0, 0, 0);
    let tracked = |e: &Entry| manifest.get(&e.dest).is_some();
    for s in &plan.steps {
        let d = env.display(&s.entry.dest);
        let rel = &s.entry.rel;
        match &s.state {
            State::Linked => linked += 1,
            State::Missing => {
                missing += 1;
                lines.push(format!(
                    "  manquant     {d}  (poste sys setup crée le lien)"
                ));
            }
            State::OtherLink {
                broken: true,
                target,
            } => {
                broken += 1;
                lines.push(format!(
                    "  lien cassé   {d}  -> {}  (poste sys setup --take-repo {rel})",
                    target.display()
                ));
            }
            State::Identical if tracked(&s.entry) => {
                replaced += 1;
                lines.push(format!(
                    "  remplacé par un fichier ordinaire (identique au dépôt)  {d}  (poste sys setup relie)"
                ));
            }
            State::Differs if tracked(&s.entry) => {
                replaced += 1;
                lines.push(format!(
                    "  remplacé par un fichier ordinaire, modifié  {d}  (une appli a réécrit le fichier : poste sys setup --take-home {rel} pour le reprendre, ou --take-repo {rel})"
                ));
            }
            State::Identical => {
                missing += 1;
                lines.push(format!(
                    "  pas encore lié  {d}  (fichier identique, poste sys setup le relie)"
                ));
            }
            other => {
                conflicts += 1;
                lines.push(format!(
                    "  conflit      {d}  [{}]  (poste sys diff {rel})",
                    describe(other)
                ));
            }
        }
    }
    println!("  liés            {linked}");
    println!("  manquants       {missing}");
    println!("  liens cassés    {broken}");
    println!("  remplacés       {replaced}  (lien devenu fichier ordinaire)");
    println!("  conflits        {conflicts}");
    println!("  droits à corriger {}", plan.modes.len());
    for l in lines {
        println!("{l}");
    }
    for m in &plan.modes {
        println!("  droits       {}  {:04o} -> {:04o}", m.rel, m.from, m.to);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Dépôt et HOME temporaires : jamais le vrai `~`.
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
    fn analyse_des_fichiers_de_metadonnees() {
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
    fn lecture_du_depot_liens_et_meta() {
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
    fn chaque_etat_de_cible() {
        let f = fx();
        f.cfg("lie", "a");
        f.cfg("absent", "a");
        f.cfg("identique", "a");
        f.cfg("different", "a");
        f.cfg("ailleurs", "a");
        f.cfg("cassé", "a");
        f.cfg("dossier", "a");
        f.cfg("sous/fichier", "a");
        symlink(f.repo.join("config/lie"), f.home("lie")).unwrap();
        f.put("identique", "a");
        f.put("different", "b");
        f.put("autre", "a");
        symlink(f.home("autre"), f.home("ailleurs")).unwrap();
        symlink(f.home("n-existe-pas"), f.home("cassé")).unwrap();
        fs::create_dir(f.home("dossier")).unwrap();
        f.put("sous", "je suis un fichier");
        assert_eq!(f.state("lie"), State::Linked);
        assert_eq!(f.state("absent"), State::Missing);
        assert_eq!(f.state("identique"), State::Identical);
        assert_eq!(f.state("different"), State::Differs);
        assert!(matches!(
            f.state("ailleurs"),
            State::OtherLink { broken: false, .. }
        ));
        assert!(matches!(
            f.state("cassé"),
            State::OtherLink { broken: true, .. }
        ));
        assert!(matches!(f.state("dossier"), State::WrongType(_)));
        assert!(matches!(f.state("sous/fichier"), State::ParentBlocked(_)));
    }

    #[test]
    fn application_puis_relance_a_jour() {
        let f = fx();
        let src = f.cfg(".config/a/x.conf", "contenu");
        f.cfg(".config/a/y.conf", "autre");
        f.put(".config/a/y.conf", "autre");
        let p = f.plan(&[]);
        assert_eq!(p.count(&Op::Create), 1);
        assert_eq!(p.count(&Op::Replace), 1);
        assert_eq!(p.writes(), 2);
        // L'aperçu n'écrit rien.
        assert!(fs::symlink_metadata(f.home(".config/a/x.conf")).is_err());
        assert!(f.backups().is_empty());
        let m = f.apply(&p);
        assert_eq!(fs::read_link(f.home(".config/a/x.conf")).unwrap(), src);
        assert!(f.home(".config/a/y.conf").is_symlink());
        // Modifier le fichier lié modifie le dépôt.
        fs::write(f.home(".config/a/x.conf"), "changé").unwrap();
        assert_eq!(fs::read_to_string(&src).unwrap(), "changé");
        // L'ancien fichier identique a été sauvegardé.
        let b = f.backups();
        assert_eq!(b.len(), 1, "{b:?}");
        assert!(b[0].starts_with(".config_a_y.conf.20260101-000000"));
        assert_eq!(
            fs::read_to_string(f.env.backup_dir().join(&b[0])).unwrap(),
            "autre"
        );
        // installed.json tient les deux entrées du module config.
        assert_eq!(m.files.iter().filter(|e| e.module == MODULE).count(), 2);
        // Relance : rien à faire.
        let p2 = f.plan(&[]);
        assert_eq!(p2.writes(), 0);
        assert_eq!(p2.count(&Op::Done), 2);
        assert!(p2.conflicts().is_empty());
    }

    #[test]
    fn conflits_listes_sans_bloquer_le_reste() {
        let f = fx();
        f.cfg("a-creer", "1");
        f.cfg("conflit", "repo");
        f.put("conflit", "maison");
        let p = f.plan(&[]);
        assert_eq!(p.conflicts().len(), 1);
        assert_eq!(p.count(&Op::Create), 1);
        f.apply(&p);
        assert!(f.home("a-creer").is_symlink());
        // Le fichier en conflit est intact, rien n'a été sauvegardé ni écrasé.
        assert_eq!(fs::read_to_string(f.home("conflit")).unwrap(), "maison");
        assert!(!f.home("conflit").is_symlink());
        assert!(f.backups().is_empty());
        assert_eq!(f.plan(&[]).conflicts().len(), 1);
    }

    #[test]
    fn decision_take_repo() {
        let f = fx();
        let src = f.cfg("c", "repo");
        f.put("c", "maison");
        f.apply(&f.plan(&[("c", Decision::Repo)]));
        assert_eq!(fs::read_link(f.home("c")).unwrap(), src);
        assert_eq!(fs::read_to_string(&src).unwrap(), "repo");
        let b = f.backups();
        assert_eq!(b.len(), 1);
        assert_eq!(
            fs::read_to_string(f.env.backup_dir().join(&b[0])).unwrap(),
            "maison"
        );
    }

    #[test]
    fn decision_take_home_recopie_dans_le_depot() {
        let f = fx();
        let src = f.cfg("c", "repo");
        let h = f.put("secret.conf", "x");
        f.cfg("secret.conf", "vieux");
        f.put("c", "maison");
        fs::set_permissions(f.home("c"), fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&h, fs::Permissions::from_mode(0o600)).unwrap();
        f.apply(&f.plan(&[("c", Decision::Home), ("secret.conf", Decision::Home)]));
        assert_eq!(fs::read_to_string(&src).unwrap(), "maison");
        assert_eq!(fs::metadata(&src).unwrap().mode() & 0o777, 0o755);
        assert!(f.home("c").is_symlink());
        assert_eq!(f.backups().len(), 2);
        // Droits restreints notés puis appliqués au fichier du dépôt.
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
        // Décision sur un chemin inconnu : refus net.
        let m: HashMap<_, _> = [("zzz".to_string(), Decision::Repo)].into();
        assert!(plan(&f.layout(), &f.env, &m).is_err());
    }

    #[test]
    fn take_home_impossible_sur_un_dossier_a_la_place_dun_fichier() {
        let f = fx();
        f.cfg("c", "repo");
        fs::create_dir(f.home("c")).unwrap();
        let m: HashMap<_, _> = [("c".to_string(), Decision::Home)].into();
        assert!(plan(&f.layout(), &f.env, &m).is_err());
        // --take-repo est possible : le dossier est déplacé en sauvegarde.
        fs::write(f.home("c/dedans"), "z").unwrap();
        f.apply(&f.plan(&[("c", Decision::Repo)]));
        assert!(f.home("c").is_symlink());
        let b = f.backups();
        assert!(f.env.backup_dir().join(&b[0]).join("dedans").is_file());
    }

    #[test]
    fn lien_ailleurs_est_un_conflit_puis_se_tranche() {
        let f = fx();
        let src = f.cfg("c", "repo");
        f.put("autre", "autre");
        symlink(f.home("autre"), f.home("c")).unwrap();
        assert_eq!(f.plan(&[]).conflicts().len(), 1);
        f.apply(&f.plan(&[("c", Decision::Repo)]));
        assert_eq!(fs::read_link(f.home("c")).unwrap(), src);
        assert_eq!(fs::read_to_string(f.home("autre")).unwrap(), "autre");
        assert_eq!(f.backups().len(), 1);
    }

    #[test]
    fn liens_symboliques_de_la_config() {
        let f = fx();
        symlink("../../.agents/skills/x", {
            let p = f.repo.join("config/.claude/skills");
            fs::create_dir_all(&p).unwrap();
            p.join("x")
        })
        .unwrap();
        f.apply(&f.plan(&[]));
        // Même cible, conservée telle quelle.
        assert_eq!(
            fs::read_link(f.home(".claude/skills/x")).unwrap(),
            PathBuf::from("../../.agents/skills/x")
        );
        assert_eq!(f.plan(&[]).writes(), 0);
        // Un lien vers ailleurs est un conflit ; --take-home reprend sa cible dans le dépôt.
        fs::remove_file(f.home(".claude/skills/x")).unwrap();
        symlink("/ailleurs", f.home(".claude/skills/x")).unwrap();
        assert_eq!(f.plan(&[]).conflicts().len(), 1);
        f.apply(&f.plan(&[(".claude/skills/x", Decision::Home)]));
        assert_eq!(
            fs::read_link(f.repo.join("config/.claude/skills/x")).unwrap(),
            PathBuf::from("/ailleurs")
        );
        assert_eq!(f.plan(&[]).writes(), 0);
    }

    #[test]
    fn dossier_lie_d_un_bloc() {
        let f = fx();
        f.cfg(".config/p/k/a.qml", "A");
        f.cfg(".config/p/k/bin/run", "R");
        f.meta("link-dirs", ".config/p/k\n");
        let l = f.layout();
        assert_eq!(l.entries.len(), 1);
        assert_eq!(l.entries[0].kind, Kind::Dir);
        // Cible absente : lien de dossier.
        f.apply(&f.plan(&[]));
        assert!(f.home(".config/p/k").is_symlink());
        assert_eq!(
            fs::read_to_string(f.home(".config/p/k/bin/run")).unwrap(),
            "R"
        );
        assert_eq!(f.plan(&[]).writes(), 0);

        // Dossier réel identique (hors __pycache__) : déplacé en sauvegarde, remplacé par le lien.
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

        // Fichier réel en plus : conflit, rien ne bouge.
        let h = fx();
        h.cfg(".config/p/k/a.qml", "A");
        h.meta("link-dirs", ".config/p/k\n");
        h.put(".config/p/k/a.qml", "A");
        h.put(".config/p/k/etat.json", "{}");
        assert_eq!(h.state(".config/p/k"), State::Differs);
        h.apply(&h.plan(&[]));
        assert!(!h.home(".config/p/k").is_symlink());
        assert!(h.home(".config/p/k/etat.json").is_file());
        // --take-home : le fichier en plus entre dans le dépôt.
        h.apply(&h.plan(&[(".config/p/k", Decision::Home)]));
        assert!(h.home(".config/p/k").is_symlink());
        assert_eq!(
            fs::read_to_string(h.repo.join("config/.config/p/k/etat.json")).unwrap(),
            "{}"
        );
    }

    #[test]
    fn link_dirs_invalides() {
        let f = fx();
        f.cfg("a/b", "x");
        f.meta("link-dirs", "absent\n");
        assert!(load(&f.repo, &f.env).is_err());
        f.meta("link-dirs", "a\na/b\n");
        assert!(load(&f.repo, &f.env).is_err());
    }

    #[test]
    fn droits_reappliques_a_la_cible_reelle() {
        let f = fx();
        let src = f.cfg(".config/foot/foot.ini", "x");
        f.cfg(".config/priv/f", "y");
        fs::create_dir_all(f.home(".config/priv")).unwrap();
        f.meta("modes", ".config/foot/foot.ini 0600\n.config/priv 0700\n");
        let p = f.plan(&[]);
        assert_eq!(p.modes.len(), 2);
        f.apply(&p);
        // Le mode est sur le fichier du dépôt (cible du lien), pas sur le lien.
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
    fn add_deplace_et_lie_avec_sauvegarde() {
        let f = fx();
        let h = f.put(".config/app/conf", "valeur");
        fs::set_permissions(&h, fs::Permissions::from_mode(0o600)).unwrap();
        let mf = f.env.manifest_file();
        add(&f.env, &f.repo, ".config/app/conf", false, &mf).unwrap();
        let in_repo = f.repo.join("config/.config/app/conf");
        assert_eq!(fs::read_to_string(&in_repo).unwrap(), "valeur");
        assert_eq!(fs::read_link(&h).unwrap(), in_repo);
        assert_eq!(f.backups().len(), 1);
        let modes =
            parse_modes(&fs::read_to_string(f.repo.join("config/.poste/modes")).unwrap()).unwrap();
        assert_eq!(modes[".config/app/conf"], 0o600);
        // Déjà géré : sans effet, sans erreur.
        add(&f.env, &f.repo, ".config/app/conf", false, &mf).unwrap();
        assert_eq!(f.backups().len(), 1);
        assert_eq!(f.plan(&[]).writes(), 0);
    }

    #[test]
    fn add_refuse_les_secrets_et_respecte_dry_run() {
        let f = fx();
        f.put(".ssh/id_ed25519", "cle");
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
        // Règle ajoutée dans le fichier `ignore`.
        f.meta("ignore", ".config/autre/**\n");
        f.put(".config/autre/f", "x");
        assert!(add(&f.env, &f.repo, ".config/autre/f", false, &mf).is_err());
        // --dry-run : rien d'écrit.
        f.put(".config/ok", "x");
        add(&f.env, &f.repo, ".config/ok", true, &mf).unwrap();
        assert!(!f.repo.join("config/.config/ok").exists());
        assert!(!f.home(".config/ok").is_symlink());
        assert!(!mf.exists());
        // Un dossier : les fichiers ignorés sont sautés, les autres ajoutés.
        f.put(".config/d/a", "1");
        f.put(".config/d/.env", "secret");
        add(&f.env, &f.repo, ".config/d", false, &mf).unwrap();
        assert!(f.repo.join("config/.config/d/a").is_file());
        assert!(!f.repo.join("config/.config/d/.env").exists());
        assert!(f.home(".config/d/.env").is_file() && !f.home(".config/d/.env").is_symlink());
    }

    #[test]
    fn forget_remet_un_fichier_ordinaire() {
        let f = fx();
        let src = f.cfg("c", "v");
        f.apply(&f.plan(&[]));
        assert!(f.home("c").is_symlink());
        let mf = f.env.manifest_file();
        forget(&f.layout(), &f.env, "c", &mf).unwrap();
        assert!(!f.home("c").is_symlink());
        assert_eq!(fs::read_to_string(f.home("c")).unwrap(), "v");
        // Le dépôt garde son fichier ; la suite le verrait comme identique.
        assert!(src.is_file());
        assert_eq!(f.state("c"), State::Identical);
        assert!(Manifest::load(&mf).unwrap().get(&f.home("c")).is_none());
        // Pas lié : refus.
        assert!(forget(&f.layout(), &f.env, "c", &mf).is_err());
    }

    #[test]
    fn chemins_donnes_par_l_utilisateur() {
        let f = fx();
        assert_eq!(rel_of(&f.env, ".config/a").unwrap(), ".config/a");
        assert_eq!(rel_of(&f.env, "~/.config/a").unwrap(), ".config/a");
        let abs = format!("{}/.config/a", f.env.home.display());
        assert_eq!(rel_of(&f.env, &abs).unwrap(), ".config/a");
        assert!(rel_of(&f.env, "/etc/passwd").is_err());
        assert!(rel_of(&f.env, "~/../x").is_err());
    }

    #[test]
    fn diff_signale_les_ecarts() {
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
    fn statut_ne_modifie_rien() {
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
