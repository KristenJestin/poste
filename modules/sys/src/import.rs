//! `poste sys import-chezmoi` : reprend dans `config/` ce que chezmoi gère, en prenant le
//! contenu RÉEL de la machine (l'état en place fait foi, pas la source chezmoi).
//! N'écrit que dans le dépôt poste et dans `~/.local/state/poste/import-chezmoi.md` ;
//! ne touche à aucun fichier de `~` ni à la source chezmoi.

use crate::config::{self, META};
use crate::ignore::Ignore;
use crate::util::{copy_tree, sha256_file};
use anyhow::{bail, Context, Result};
use poste_core::Env;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Dossiers que l'on voudrait lier d'un bloc, s'ils sont propres.
const PROPOSED_LINK_DIRS: &[&str] = &[".config/omarchy/plugins/poste.island"];
/// Dossiers dont on contrôle la propreté et que l'on signale (en plus des proposés).
const CANDIDATE_PARENTS: &[&str] = &[".config/omarchy/plugins"];
const CANDIDATE_DIRS: &[&str] = &[".config/poste-island"];

const IGNORE_TEMPLATE: &str = "# Chemins (relatifs à ~) que `poste sys add` refuse d'ajouter.\n\
# Repris des règles utiles de l'ancien .chezmoiignore. `*` et `?` ne franchissent pas un `/`,\n\
# `**` couvre zéro dossier ou plus ; un motif qui correspond à un dossier couvre son contenu.\n\
\n\
# Secrets : ne jamais versionner\n\
.ssh/**\n\
.gnupg/**\n\
.npmrc\n\
.netrc\n\
.docker/.token_seed\n\
.claude/.credentials.json\n\
**/.env\n\
**/.env.*\n\
.config/configstore/**\n\
\n\
# Caches et fichiers régénérables\n\
.cache/**\n\
.npm/**\n\
.dotnet/**\n\
.nuget/**\n\
**/__pycache__/**\n\
**/*.bak.*\n\
**/*.bak\n\
\n\
# Géré ou régénéré par Omarchy et des outils tiers\n\
.config/omarchy/current\n\
.claude/skills/omarchy\n\
.claude/plugins\n";

// ---------------------------------------------------------------- source chezmoi

const PREFIXES: &[&str] = &[
    "after_",
    "before_",
    "create_",
    "empty_",
    "encrypted_",
    "exact_",
    "executable_",
    "literal_",
    "modify_",
    "once_",
    "onchange_",
    "private_",
    "readonly_",
    "remove_",
    "run_",
    "symlink_",
    "dot_",
];

#[derive(Debug, Default, Clone, Copy)]
struct Attrs {
    exec: bool,
    private: bool,
    symlink: bool,
}

/// Attributs portés par les préfixes d'un chemin de la source (tous ses composants).
fn attrs_of(rel_src: &Path) -> Attrs {
    let mut a = Attrs::default();
    for c in rel_src.components() {
        let mut name = c.as_os_str().to_string_lossy().into_owned();
        'again: loop {
            for p in PREFIXES {
                if let Some(rest) = name.strip_prefix(p) {
                    match *p {
                        "executable_" => a.exec = true,
                        "private_" => a.private = true,
                        "symlink_" => a.symlink = true,
                        _ => {}
                    }
                    name = rest.to_string();
                    continue 'again;
                }
            }
            break;
        }
    }
    a
}

fn chezmoi(env: &Env, source: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("chezmoi")
        .env("HOME", &env.home)
        .arg("--source")
        .arg(source)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .context("impossible de lancer chezmoi")?;
    if !out.status.success() {
        bail!(
            "chezmoi {} a échoué : {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn lines(s: &str) -> Vec<String> {
    s.lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Chemins de la source pour des cibles (même ordre).
fn source_paths(env: &Env, source: &Path, targets: &[String]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for chunk in targets.chunks(80) {
        // chezmoi veut des chemins absolus (sinon relatifs au dossier courant).
        let abs: Vec<String> = chunk
            .iter()
            .map(|t| env.home.join(t).to_string_lossy().into_owned())
            .collect();
        let mut args: Vec<&str> = vec!["source-path"];
        args.extend(abs.iter().map(String::as_str));
        let got = lines(&chezmoi(env, source, &args)?);
        if got.len() != chunk.len() {
            bail!("chezmoi source-path : réponse inattendue");
        }
        out.extend(got.into_iter().map(PathBuf::from));
    }
    Ok(out)
}

// ---------------------------------------------------------------- recherche de secrets

#[derive(Debug, Default)]
pub struct Scan {
    /// (numéro de ligne, nature) ; jamais la valeur.
    pub hits: Vec<(usize, &'static str)>,
    /// Lignes qui mentionnent un mot-clé sans valeur évidente.
    pub mentions: usize,
}

const KEYWORDS: &[(&str, usize)] = &[
    ("password", 6),
    ("passwd", 6),
    ("secret", 16),
    ("token", 16),
    ("api_key", 16),
    ("apikey", 16),
    ("api-key", 16),
    ("access_key", 16),
    ("private_key", 16),
    ("authorization", 16),
];

fn value_chars(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_-./+=~@!#%^*".contains(c)
}

/// Valeur littérale qui suit un mot-clé (`token: "abc…"`, `PASSWORD=xyz`) ; `None` sinon.
fn assigned_value(rest: &str) -> Option<(&str, bool, Option<char>)> {
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let rest = rest.trim_start_matches(['"', '\'', ' ', '\t']);
    let rest = rest.strip_prefix(['=', ':'])?;
    let before = rest;
    let rest = rest.trim_start_matches(['"', '\'', ' ', '\t', '=']);
    let quoted = before[..before.len() - rest.len()].contains(['"', '\'']);
    let end = rest
        .char_indices()
        .find(|(_, c)| !value_chars(*c))
        .map_or(rest.len(), |(i, _)| i);
    Some((&rest[..end], quoted, rest[end..].chars().next()))
}

fn known_token(line: &str) -> Option<&'static str> {
    let run = |prefix: &str, min: usize, pred: fn(char) -> bool| {
        line.match_indices(prefix).any(|(i, _)| {
            line[i + prefix.len()..]
                .chars()
                .take_while(|c| pred(*c))
                .count()
                >= min
        })
    };
    let alnum = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    if run("ghp_", 30, alnum) || run("github_pat_", 30, alnum) || run("glpat-", 18, alnum) {
        Some("jeton GitHub/GitLab")
    } else if run("sk-", 24, alnum) {
        Some("clé d'API (sk-)")
    } else if run("xoxb-", 10, alnum) || run("xoxp-", 10, alnum) {
        Some("jeton Slack")
    } else if line.match_indices("AKIA").any(|(i, _)| {
        line[i + 4..]
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            .count()
            >= 16
    }) {
        Some("clé AWS")
    } else if run("eyJ", 20, alnum) && line.contains('.') && line.matches("eyJ").count() >= 1 && {
        // Trois segments séparés par des points : forme d'un JWT.
        line.split(|c: char| !(c.is_ascii_alphanumeric() || "_-.".contains(c)))
            .any(|w| w.starts_with("eyJ") && w.matches('.').count() == 2)
    } {
        Some("jeton JWT")
    } else {
        None
    }
}

pub fn scan_secrets(text: &str) -> Scan {
    let mut s = Scan::default();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        if line.contains("-----BEGIN") && line.contains("PRIVATE KEY") {
            s.hits.push((n, "clé privée"));
            continue;
        }
        if let Some(k) = known_token(line) {
            s.hits.push((n, k));
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(p) = lower.find("bearer ") {
            let tok: String = line[p + 7..]
                .chars()
                .skip_while(|c| *c == ' ')
                .take_while(|c| value_chars(*c))
                .collect();
            if tok.len() >= 16 && tok.chars().any(|c| c.is_ascii_digit()) {
                s.hits.push((n, "jeton Bearer"));
                continue;
            }
        }
        let mut mentioned = false;
        let mut hit = None;
        for (kw, min) in KEYWORDS {
            for (p, _) in lower.match_indices(kw) {
                mentioned = true;
                if let Some((v, quoted, next)) = assigned_value(&line[p + kw.len()..]) {
                    // Une expression de code (appel, accès à un membre) n'est pas un secret.
                    let code = !quoted && (next == Some('(') || v.contains('.') && v.len() < 24);
                    let literal = !code
                        && v.len() >= *min
                        && !v.starts_with(['$', '{', '<', '%', '/', '~', '.'])
                        && (*min == 6 || v.chars().any(|c| c.is_ascii_digit()));
                    if literal {
                        hit = Some(*kw);
                    }
                }
            }
        }
        if let Some(kw) = hit {
            let _ = kw;
            s.hits.push((
                n,
                "valeur littérale après un mot-clé (token, password, secret, api_key…)",
            ));
        } else if mentioned {
            s.mentions += 1;
        }
    }
    s
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|b| *b == 0)
}

// ---------------------------------------------------------------- rapport

#[derive(Default)]
struct Report {
    imported: usize,
    imported_links: usize,
    already_linked: Vec<String>,
    differs: Vec<(String, String)>,
    absent: Vec<String>,
    mismatched: Vec<(String, String)>,
    secrets: Vec<(String, Vec<(usize, &'static str)>)>,
    ignored: Vec<(String, String)>,
    review: Vec<(String, usize)>,
    notes: Vec<String>,
    modes: BTreeMap<String, u32>,
    absent_dirs: Vec<String>,
    da: Vec<String>,
    candidates: Vec<(String, Vec<String>, bool)>,
    link_dirs: Vec<String>,
    bootstrap: Vec<String>,
}

/// Début d'un diff entre la source chezmoi et la machine, plafonné.
fn short_diff(src_file: &Path, real: &Path) -> String {
    const MAX: usize = 40;
    let out = Command::new("diff")
        .args(["-u", "--label", "chezmoi", "--label", "machine"])
        .arg(src_file)
        .arg(real)
        .output();
    let Ok(out) = out else {
        return "(diff indisponible)".into();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    if text.starts_with("Binary files") || text.is_empty() {
        return format!(
            "(fichiers binaires, tailles {} / {} octets)",
            std::fs::metadata(src_file).map_or(0, |m| m.len()),
            std::fs::metadata(real).map_or(0, |m| m.len())
        );
    }
    let all: Vec<&str> = text.lines().collect();
    let mut s = all.iter().take(MAX).copied().collect::<Vec<_>>().join("\n");
    if all.len() > MAX {
        let _ = write!(s, "\n… ({} ligne(s) de plus)", all.len() - MAX);
    }
    s
}

fn git_note(source: &Path, rel_src: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(source)
        .args([
            "log",
            "--diff-filter=D",
            "--format=%h %ad %s",
            "--date=short",
            "-3",
            "--",
        ])
        .arg(rel_src)
        .output()
        .ok()?;
    let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!t.is_empty()).then_some(t)
}

fn count_files(dir: &Path) -> usize {
    let mut n = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => n += count_files(&e.path()),
                Ok(_) => n += 1,
                Err(_) => {}
            }
        }
    }
    n
}

/// Fichiers et liens réels (relatifs à `~`) sous `dir`, hors `__pycache__`.
fn real_files(home: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        if e.file_name() == "__pycache__" {
            continue;
        }
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => real_files(home, &p, out),
            Ok(_) => out.push(p.strip_prefix(home).unwrap().to_string_lossy().into_owned()),
            Err(_) => {}
        }
    }
}

fn restricted(mode: u32) -> bool {
    mode & 0o077 == 0
}

pub fn run(env: &Env, repo: &Path, source: Option<&Path>, dry_run: bool) -> Result<()> {
    let source: PathBuf = match source {
        Some(s) => s.to_path_buf(),
        None => {
            let t = chezmoi(env, &env.data_home.join("chezmoi"), &["source-path"])
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if t.is_empty() {
                env.data_home.join("chezmoi")
            } else {
                PathBuf::from(t)
            }
        }
    };
    if !source.is_dir() {
        bail!("source chezmoi introuvable : {}", source.display());
    }
    let config_root = repo.join("config");
    let ignore = Ignore::load(&config_root.join(META).join("ignore"))?;

    let files = lines(&chezmoi(
        env,
        &source,
        &["managed", "--include=files,symlinks"],
    )?);
    let dirs = lines(&chezmoi(env, &source, &["managed", "--include=dirs"])?);
    let file_src = source_paths(env, &source, &files)?;
    let dir_src = source_paths(env, &source, &dirs)?;
    let managed: BTreeSet<&str> = files.iter().map(String::as_str).collect();

    let mut r = Report::default();
    // Cibles qui n'ont pas pu être importées (pour juger de la propreté d'un dossier).
    let mut not_imported: BTreeSet<String> = BTreeSet::new();
    let mut to_write: Vec<(String, Vec<u8>, bool)> = Vec::new();
    let mut links_to_write: Vec<(String, PathBuf)> = Vec::new();

    for (target, srcp) in files.iter().zip(&file_src) {
        let rel_src = srcp.strip_prefix(&source).unwrap_or(srcp);
        let attrs = attrs_of(rel_src);
        let real = env.home.join(target);
        if let Some(rule) = ignore.matching(target) {
            r.ignored.push((target.clone(), rule.to_string()));
            not_imported.insert(target.clone());
            continue;
        }
        let md = match std::fs::symlink_metadata(&real) {
            Ok(m) => m,
            Err(_) => {
                r.absent.push(target.clone());
                not_imported.insert(target.clone());
                continue;
            }
        };
        // Déjà un lien vers config/ (relance après un setup) : on ne relit pas son propre dépôt.
        if md.file_type().is_symlink() {
            if let Ok(t) = std::fs::canonicalize(&real) {
                if std::fs::canonicalize(&config_root).is_ok_and(|c| t.starts_with(c)) {
                    r.already_linked.push(target.clone());
                    continue;
                }
            }
        }
        if attrs.symlink {
            let want = std::fs::read_to_string(srcp).unwrap_or_default();
            let want = want.trim_end_matches('\n');
            if !md.file_type().is_symlink() {
                r.mismatched.push((
                    target.clone(),
                    "chezmoi attend un lien, la machine a autre chose".into(),
                ));
                not_imported.insert(target.clone());
                continue;
            }
            let got = std::fs::read_link(&real)?;
            if got.to_string_lossy() != want {
                r.differs.push((
                    target.clone(),
                    format!("cible chezmoi : {want}\ncible machine : {}", got.display()),
                ));
            }
            links_to_write.push((target.clone(), got));
            r.imported_links += 1;
            continue;
        }
        if md.file_type().is_symlink() || !md.is_file() {
            r.mismatched.push((
                target.clone(),
                "chezmoi attend un fichier ordinaire, la machine a un lien ou un dossier".into(),
            ));
            not_imported.insert(target.clone());
            continue;
        }
        let bytes =
            std::fs::read(&real).with_context(|| format!("lecture de {}", real.display()))?;
        let mut secret_found = false;
        if !is_binary(&bytes) {
            let sc = scan_secrets(&String::from_utf8_lossy(&bytes));
            if !sc.hits.is_empty() {
                r.secrets.push((target.clone(), sc.hits));
                not_imported.insert(target.clone());
                secret_found = true;
            } else if sc.mentions > 0 {
                r.review.push((target.clone(), sc.mentions));
            }
        }
        if secret_found {
            continue;
        }
        // Écart avec la source chezmoi.
        let src_bytes = std::fs::read(srcp).unwrap_or_default();
        if src_bytes != bytes {
            r.differs.push((target.clone(), short_diff(srcp, &real)));
        }
        let mode = md.mode() & 0o777;
        let real_exec = mode & 0o111 != 0;
        if attrs.exec != real_exec {
            r.notes.push(format!(
                "{target} : préfixe chezmoi {} mais le fichier réel {} exécutable (le réel est retenu)",
                if attrs.exec { "executable_" } else { "sans executable_" },
                if real_exec { "est" } else { "n'est pas" }
            ));
        }
        if attrs.private != restricted(mode) {
            r.notes.push(if attrs.private {
                format!("{target} : préfixe private_ mais droits réels {mode:04o} (non restreints, rien noté dans modes)")
            } else {
                format!("{target} : droits réels {mode:04o} sans préfixe private_ (noté dans modes)")
            });
        }
        if restricted(mode) {
            r.modes.insert(target.clone(), mode);
        }
        to_write.push((target.clone(), bytes, real_exec));
        r.imported += 1;
    }

    // Dossiers gérés : droits restreints, et dossiers absents (entrées DA de chezmoi status).
    for (target, srcp) in dirs.iter().zip(&dir_src) {
        let rel_src = srcp.strip_prefix(&source).unwrap_or(srcp);
        let attrs = attrs_of(rel_src);
        let real = env.home.join(target);
        match std::fs::symlink_metadata(&real) {
            Ok(m) if m.is_dir() => {
                let mode = m.mode() & 0o777;
                if restricted(mode) {
                    r.modes.insert(target.clone(), mode);
                } else if attrs.private {
                    r.notes.push(format!(
                        "{target}/ : préfixe private_ mais droits réels {mode:04o}"
                    ));
                }
            }
            Ok(_) => r
                .mismatched
                .push((format!("{target}/"), "chezmoi attend un dossier".into())),
            Err(_) => {
                let has_managed = managed.iter().any(|f| f.starts_with(&format!("{target}/")));
                if !has_managed {
                    // Dossier de la source sans aucun fichier géré, absent de la machine.
                    let n = count_files(srcp);
                    let hist = git_note(&source, rel_src);
                    let mut d = format!(
                        "{target}/ : dossier absent de la machine ; dans la source, {} ({n} fichier(s)) ",
                        srcp.strip_prefix(&source).unwrap_or(srcp).display()
                    );
                    d.push_str(if n == 0 {
                        "est un dossier VIDE (reliquat : ses fichiers ont été retirés du dépôt chezmoi, git ne garde pas les dossiers vides)"
                    } else {
                        "contient encore des fichiers non gérés"
                    });
                    if let Some(h) = hist {
                        let _ = write!(d, ". Fichiers retirés par : {}", h.replace('\n', " ; "));
                    }
                    r.da.push(d);
                } else {
                    r.absent_dirs.push(target.clone());
                }
            }
        }
    }

    // Propreté des dossiers candidats au lien de dossier.
    let mut cand: BTreeSet<String> = CANDIDATE_DIRS.iter().map(|s| (*s).to_string()).collect();
    for p in CANDIDATE_PARENTS {
        if let Ok(rd) = std::fs::read_dir(env.home.join(p)) {
            for e in rd.flatten() {
                let rel = format!("{p}/{}", e.file_name().to_string_lossy());
                if e.path().is_dir() && managed.iter().any(|f| f.starts_with(&format!("{rel}/"))) {
                    cand.insert(rel);
                }
            }
        }
    }
    for c in PROPOSED_LINK_DIRS {
        cand.insert((*c).to_string());
    }
    for c in cand {
        let mut real = Vec::new();
        real_files(&env.home, &env.home.join(&c), &mut real);
        let unmanaged: Vec<String> = real
            .iter()
            .filter(|f| !managed.contains(f.as_str()))
            .cloned()
            .collect();
        let skipped = managed
            .iter()
            .any(|f| f.starts_with(&format!("{c}/")) && not_imported.contains(*f));
        let clean = unmanaged.is_empty() && !skipped;
        if clean && PROPOSED_LINK_DIRS.contains(&c.as_str()) {
            r.link_dirs.push(c.clone());
        }
        r.candidates.push((c, unmanaged, skipped));
    }

    // Fichiers du dépôt chezmoi qui ne sont pas de la config : vers bootstrap/.
    for name in ["README.md", "bootstrap.sh", "bootstrap-patches"] {
        if source.join(name).exists() {
            r.bootstrap.push(name.to_string());
        }
    }

    // ---- écritures (dépôt poste seulement)
    if !dry_run {
        for (target, bytes, exec) in &to_write {
            let dst = config_root.join(target);
            if let Some(p) = dst.parent() {
                std::fs::create_dir_all(p)?;
            }
            if std::fs::symlink_metadata(&dst)
                .is_ok_and(|m| m.file_type().is_symlink() || m.is_dir())
            {
                bail!(
                    "{} existe dans config/ avec un autre type, abandon",
                    dst.display()
                );
            }
            let tmp = dst.with_file_name(format!(".poste-tmp-{}-import", std::process::id()));
            std::fs::write(&tmp, bytes)?;
            std::fs::set_permissions(
                &tmp,
                std::fs::Permissions::from_mode(if *exec { 0o755 } else { 0o644 }),
            )?;
            std::fs::rename(&tmp, &dst)?;
            if sha256_file(&dst)? != sha256_file(&env.home.join(target))? {
                bail!("copie de {target} différente de l'original, abandon");
            }
        }
        for (target, link_to) in &links_to_write {
            let dst = config_root.join(target);
            if let Some(p) = dst.parent() {
                std::fs::create_dir_all(p)?;
            }
            if std::fs::symlink_metadata(&dst).is_ok_and(|m| m.file_type().is_symlink()) {
                std::fs::remove_file(&dst)?;
            }
            symlink(link_to, &dst).with_context(|| format!("lien {}", dst.display()))?;
        }
        let meta = config_root.join(META);
        std::fs::create_dir_all(&meta)?;
        if !meta.join("ignore").exists() {
            std::fs::write(meta.join("ignore"), IGNORE_TEMPLATE)?;
        }
        // Les droits restreints des fichiers importés (fichier modes : ajout/remplacement).
        for (p, m) in &r.modes {
            config::upsert_mode_line(&config_root, p, *m)?;
        }
        if !r.link_dirs.is_empty() {
            let f = meta.join("link-dirs");
            let mut cur = std::fs::read_to_string(&f).unwrap_or_default();
            for d in &r.link_dirs {
                if !cur.lines().any(|l| l.trim() == d) {
                    if !cur.is_empty() && !cur.ends_with('\n') {
                        cur.push('\n');
                    }
                    cur.push_str(d);
                    cur.push('\n');
                }
            }
            std::fs::write(&f, cur)?;
        }
        if !r.bootstrap.is_empty() {
            let b = repo.join("bootstrap");
            std::fs::create_dir_all(&b)?;
            for name in &r.bootstrap {
                let from = source.join(name);
                if from.is_dir() {
                    copy_tree(&from, &b.join(name))?;
                } else {
                    std::fs::copy(&from, b.join(name))?;
                }
            }
        }
        ensure_gitignore(repo)?;
    }

    let text = render(&r, files.len(), dirs.len(), &source);
    if dry_run {
        println!("{text}");
        println!("\nSimulation (--dry-run) : rien n'a été écrit.");
    } else {
        let f = env.state_dir().join("import-chezmoi.md");
        std::fs::create_dir_all(env.state_dir())?;
        std::fs::write(&f, &text)?;
        println!(
            "Import terminé : {} fichier(s) et {} lien(s) importés, {} refusé(s) pour secret, {} absent(s) de la machine, {} différent(s) de la source chezmoi.\nRapport : {}",
            r.imported,
            r.imported_links,
            r.secrets.len(),
            r.absent.len(),
            r.differs.len(),
            env.display(&f)
        );
    }
    Ok(())
}

/// `__pycache__` ne doit jamais entrer dans le dépôt (un dossier lié d'un bloc en reçoit).
fn ensure_gitignore(repo: &Path) -> Result<()> {
    let f = repo.join(".gitignore");
    let mut cur = std::fs::read_to_string(&f).unwrap_or_default();
    let mut changed = false;
    for l in ["__pycache__/", "*.pyc"] {
        if !cur.lines().any(|x| x.trim() == l) {
            if !cur.is_empty() && !cur.ends_with('\n') {
                cur.push('\n');
            }
            cur.push_str(l);
            cur.push('\n');
            changed = true;
        }
    }
    if changed {
        std::fs::write(&f, cur)?;
    }
    Ok(())
}

fn render(r: &Report, nfiles: usize, ndirs: usize, source: &Path) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Import depuis chezmoi\n");
    let _ = writeln!(
        s,
        "Source chezmoi : `{}` ({nfiles} fichier(s) et lien(s) gérés, {ndirs} dossier(s)). Le contenu importé est celui de la machine (`~`), pas celui de la source.\n",
        source.display()
    );
    let _ = writeln!(s, "## Résumé\n");
    let _ = writeln!(
        s,
        "- importés : {} fichier(s) et {} lien(s) symbolique(s) dans `config/`",
        r.imported, r.imported_links
    );
    let _ = writeln!(s, "- différents de la source chezmoi : {}", r.differs.len());
    let _ = writeln!(
        s,
        "- gérés mais absents de la machine (non importés) : {}",
        r.absent.len()
    );
    let _ = writeln!(s, "- refusés pour secret : {}", r.secrets.len());
    let _ = writeln!(
        s,
        "- ignorés par les règles d'exclusion : {}",
        r.ignored.len()
    );
    let _ = writeln!(
        s,
        "- de type différent (non importés) : {}",
        r.mismatched.len()
    );
    let _ = writeln!(
        s,
        "- déjà liés vers `config/` (non relus) : {}",
        r.already_linked.len()
    );
    let _ = writeln!(
        s,
        "- droits restreints notés dans `config/.poste/modes` : {}",
        r.modes.len()
    );
    let _ = writeln!(
        s,
        "- entrées `DA` de `chezmoi status` expliquées : {}\n",
        r.da.len()
    );

    let _ = writeln!(
        s,
        "## Fichiers dont la machine diffère de la source chezmoi\n"
    );
    if r.differs.is_empty() {
        let _ = writeln!(s, "Aucun.\n");
    }
    for (t, d) in &r.differs {
        let _ = writeln!(s, "### `{t}`\n\n```diff\n{d}\n```\n");
    }

    let _ = writeln!(
        s,
        "## Gérés par chezmoi mais absents de la machine (non importés)\n"
    );
    if r.absent.is_empty() && r.absent_dirs.is_empty() {
        let _ = writeln!(s, "Aucun.\n");
    }
    for t in &r.absent {
        let _ = writeln!(s, "- `{t}`");
    }
    for t in &r.absent_dirs {
        let _ = writeln!(s, "- dossier `{t}/` (ses fichiers sont listés ci-dessus)");
    }
    let _ = writeln!(s);

    let _ = writeln!(s, "## Entrées `DA` de `chezmoi status`\n");
    if r.da.is_empty() {
        let _ = writeln!(s, "Aucune.\n");
    } else {
        let _ = writeln!(s, "`D` : absent de la machine ; `A` : `chezmoi apply` le créerait. Ce sont des dossiers :\n");
        for d in &r.da {
            let _ = writeln!(s, "- {d}");
        }
        let _ = writeln!(s, "\nAucun contenu n'est perdu ni à importer : ces dossiers ne portent plus aucun fichier.\n");
    }

    let _ = writeln!(s, "## Refusés pour secret (valeurs jamais recopiées)\n");
    if r.secrets.is_empty() {
        let _ = writeln!(s, "Aucun.\n");
    }
    for (t, hits) in &r.secrets {
        let _ = writeln!(s, "- `{t}` : non importé");
        for (n, why) in hits.iter().take(10) {
            let _ = writeln!(s, "  - ligne {n} : {why}");
        }
        if hits.len() > 10 {
            let _ = writeln!(s, "  - … et {} autre(s)", hits.len() - 10);
        }
    }
    if !r.secrets.is_empty() {
        let _ = writeln!(s, "\nCes fichiers restent en place dans `~`, sans lien ni copie. À décider : les nettoyer (extraire la valeur vers le trousseau), ou les ajouter tels quels si la détection est un faux positif.");
    }
    let _ = writeln!(s);

    let _ = writeln!(s, "## Fichiers importés qui mentionnent un mot-clé sensible sans valeur évidente (à relire)\n");
    if r.review.is_empty() {
        let _ = writeln!(s, "Aucun.\n");
    }
    for (t, n) in &r.review {
        let _ = writeln!(s, "- `{t}` : {n} ligne(s)");
    }
    let _ = writeln!(s);

    if !r.ignored.is_empty() {
        let _ = writeln!(s, "## Ignorés par `config/.poste/ignore`\n");
        for (t, rule) in &r.ignored {
            let _ = writeln!(s, "- `{t}` (règle `{rule}`)");
        }
        let _ = writeln!(s);
    }
    if !r.mismatched.is_empty() {
        let _ = writeln!(
            s,
            "## Type différent entre chezmoi et la machine (non importés)\n"
        );
        for (t, w) in &r.mismatched {
            let _ = writeln!(s, "- `{t}` : {w}");
        }
        let _ = writeln!(s);
    }
    if !r.notes.is_empty() {
        let _ = writeln!(
            s,
            "## Droits : écarts entre préfixes chezmoi et fichiers réels\n"
        );
        for n in &r.notes {
            let _ = writeln!(s, "- {n}");
        }
        let _ = writeln!(s);
    }
    if !r.modes.is_empty() {
        let _ = writeln!(s, "## Droits restreints notés (`config/.poste/modes`)\n");
        for (p, m) in &r.modes {
            let _ = writeln!(s, "- `{p}` {m:04o}");
        }
        let _ = writeln!(s);
    }

    let _ = writeln!(s, "## Dossiers candidats au lien de dossier\n");
    let _ = writeln!(
        s,
        "Un fichier réel non géré (hors `__pycache__`) empêche le lien de dossier.\n"
    );
    for (c, unmanaged, skipped) in &r.candidates {
        if unmanaged.is_empty() && !skipped {
            let _ = writeln!(
                s,
                "- `{c}` : propre{}",
                if r.link_dirs.contains(c) {
                    " ; ajouté à `config/.poste/link-dirs`"
                } else {
                    " ; non proposé (à décider)"
                }
            );
        } else {
            let _ = writeln!(s, "- `{c}` : NON propre, lien de dossier impossible");
            if *skipped {
                let _ = writeln!(s, "  - un fichier géré n'a pas pu être importé");
            }
            for u in unmanaged.iter().take(8) {
                let _ = writeln!(s, "  - fichier réel non géré : `{u}`");
            }
            if unmanaged.len() > 8 {
                let _ = writeln!(s, "  - … et {} autre(s)", unmanaged.len() - 8);
            }
        }
    }
    let _ = writeln!(s);

    let _ = writeln!(s, "## Fichiers du dépôt chezmoi hors config\n");
    if r.bootstrap.is_empty() {
        let _ = writeln!(s, "Aucun.");
    } else {
        let _ = writeln!(
            s,
            "Copiés dans `bootstrap/` du dépôt poste : {}. Ce sont des scripts d'installation d'applis (et leurs correctifs) : à intégrer plus tard à `poste sys setup`. Ils ne sont pas dans `config/`.",
            r.bootstrap.iter().map(|b| format!("`{b}`")).collect::<Vec<_>>().join(", ")
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn attributs_des_prefixes() {
        let a = attrs_of(Path::new("dot_config/private_foot/executable_dot_run"));
        assert!(a.exec && a.private && !a.symlink);
        let a = attrs_of(Path::new("dot_claude/skills/symlink_kris"));
        assert!(a.symlink && !a.exec);
        let a = attrs_of(Path::new("dot_bashrc"));
        assert!(!a.exec && !a.private);
    }

    #[test]
    fn secrets_evidents_detectes() {
        let k = format!("-----BEGIN {} PRIVATE KEY-----", "RSA");
        assert!(!scan_secrets(&k).hits.is_empty());
        assert!(!scan_secrets("password=hunter22").hits.is_empty());
        assert!(!scan_secrets("\"api_key\": \"abcd1234efgh5678ijkl\"")
            .hits
            .is_empty());
        assert!(!scan_secrets("Authorization: Bearer abcdef1234567890abcd")
            .hits
            .is_empty());
        let gh = format!("x = ghp_{}", "a1".repeat(20));
        assert!(!scan_secrets(&gh).hits.is_empty());
    }

    #[test]
    fn faux_positifs_courants_evites() {
        for l in [
            "token = $TOKEN",
            "password = \"\"",
            "local token = getToken()",
            "// le token expire",
            "\"token_file\": \"/home/x/.config/token\"",
            "secret: {{ .secret }}",
            "bind = SUPER, T, exec, foo",
        ] {
            assert!(scan_secrets(l).hits.is_empty(), "{l}");
        }
        assert_eq!(scan_secrets("// le token expire").mentions, 1);
        assert!(scan_secrets("secret = base64.b64encode(x)").hits.is_empty());
        assert!(scan_secrets("passwordFor = n.ssid").hits.is_empty());
    }

    #[test]
    fn modele_d_ignore_est_valide() {
        let rules = crate::ignore::parse(IGNORE_TEMPLATE);
        assert!(rules.contains(&".ssh/**".to_string()));
        assert!(crate::ignore::matches("**/.env.*", "a/.env.prod"));
    }

    fn chezmoi_present() -> bool {
        Command::new("chezmoi").arg("--version").output().is_ok()
    }

    /// Source chezmoi factice et HOME temporaire : le vrai `~` n'est jamais lu.
    #[test]
    fn import_complet_dans_un_home_temporaire() {
        if !chezmoi_present() {
            return;
        }
        use std::fs;
        let d = tempfile::tempdir().unwrap();
        let (home, src, repo) = (
            d.path().join("home"),
            d.path().join("src"),
            d.path().join("repo"),
        );
        for p in [&home, &src, &repo] {
            fs::create_dir_all(p).unwrap();
        }
        let put = |root: &Path, rel: &str, c: &str| {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, c).unwrap();
            p
        };
        // Source chezmoi.
        put(&src, "dot_config/app/conf", "version=source\n");
        put(&src, "dot_config/app/executable_run", "#!/bin/sh\n");
        put(&src, "dot_config/private_priv/f", "x\n");
        put(&src, "dot_config/absent", "a\n");
        put(&src, "dot_config/leak", "sans secret\n");
        put(&src, "dot_config/skills/symlink_x", "../cible\n");
        put(&src, "README.md", "doc\n");
        put(&src, "bootstrap.sh", "#!/bin/sh\n");
        put(&src, ".chezmoiignore", "README.md\nbootstrap.sh\n");
        fs::create_dir_all(src.join("dot_config/vide.d")).unwrap();
        // État réel de la machine.
        put(&home, ".config/app/conf", "version=machine\n");
        let runf = put(&home, ".config/app/run", "#!/bin/sh\n");
        fs::set_permissions(&runf, fs::Permissions::from_mode(0o755)).unwrap();
        let f = put(&home, ".config/priv/f", "x\n");
        fs::set_permissions(&f, fs::Permissions::from_mode(0o600)).unwrap();
        fs::set_permissions(home.join(".config/priv"), fs::Permissions::from_mode(0o700)).unwrap();
        put(
            &home,
            ".config/leak",
            &format!("password={}\n", "hunter22xyz"),
        );
        symlink("../cible", {
            fs::create_dir_all(home.join(".config/skills")).unwrap();
            home.join(".config/skills/x")
        })
        .unwrap();
        let env = Env::with_home(&home);
        let before = std::fs::read_to_string(home.join(".config/app/conf")).unwrap();

        run_import(&env, &repo, &src);

        let cfg = repo.join("config");
        assert_eq!(
            fs::read_to_string(cfg.join(".config/app/conf")).unwrap(),
            "version=machine\n"
        );
        assert_eq!(
            fs::metadata(cfg.join(".config/app/run")).unwrap().mode() & 0o111,
            0o111
        );
        assert_eq!(
            fs::read_link(cfg.join(".config/skills/x")).unwrap(),
            PathBuf::from("../cible")
        );
        assert!(!cfg.join(".config/leak").exists(), "secret importé");
        assert!(!cfg.join(".config/absent").exists());
        let modes = fs::read_to_string(cfg.join(".poste/modes")).unwrap();
        assert!(modes.contains(".config/priv/f 0600") && modes.contains(".config/priv 0700"));
        assert!(cfg.join(".poste/ignore").is_file());
        assert!(
            repo.join("bootstrap/README.md").is_file()
                && repo.join("bootstrap/bootstrap.sh").is_file()
        );
        let rapport = fs::read_to_string(env.state_dir().join("import-chezmoi.md")).unwrap();
        assert!(rapport.contains("`.config/app/conf`") && rapport.contains("version=machine"));
        assert!(rapport.contains("`.config/absent`"));
        assert!(rapport.contains("`.config/leak`"));
        assert!(
            !rapport.contains("hunter22xyz"),
            "valeur de secret recopiée"
        );
        assert!(rapport.contains("vide.d"));
        // Rien n'a bougé dans HOME.
        assert_eq!(
            std::fs::read_to_string(home.join(".config/app/conf")).unwrap(),
            before
        );
        assert!(!home.join(".config/app/conf").is_symlink());

        // --dry-run sur un autre dépôt : rien d'écrit.
        let repo2 = d.path().join("repo2");
        fs::create_dir_all(&repo2).unwrap();
        run(&env, &repo2, Some(&src), true).unwrap();
        assert!(!repo2.join("config").exists() && !repo2.join("bootstrap").exists());
    }

    fn run_import(env: &Env, repo: &Path, src: &Path) {
        run(env, repo, Some(src), false).unwrap();
    }
}
