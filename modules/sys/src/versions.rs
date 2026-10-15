//! Versions installées et racine de poste.
//!
//! Tout ce que `setup` pose se lit dans une seule racine, le lien `~/.local/share/poste/current` :
//! une version installée (`~/.local/share/poste/versions/<commit>/`, archive construite par la CI)
//! ou le clone du dépôt (mode dev, clé `repo`). Une version a la disposition du dépôt :
//! `config/`, `modules/`, `bootstrap/`, les binaires dans `target/release/`, plus `VERSION`
//! (`commit=…`, `date=…`) ; sans `Cargo.toml`, `setup` ne construit rien.

use crate::reload::Need;
use crate::util::{self, sha256_file};
use crate::{machine, setup};
use anyhow::{bail, Context, Result};
use poste_core::{Config, Env, Source};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Fichier d'identité d'une version, à sa racine (écrit par la CI).
pub const VERSION_FILE: &str = "VERSION";
/// Somme de contrôle de l'archive d'où vient une version (écrite à l'installation).
pub const SUM_FILE: &str = "ARCHIVE_SHA256";
/// Nom de l'archive publiée, et de sa somme (`<archive>.sha256`).
pub const ARCHIVE: &str = "poste-x86_64-linux.tar.gz";
/// Où la CI publie l'archive : une seule préversion `latest`, remplacée à chaque push sur `main`.
pub const DEFAULT_URL: &str = "https://github.com/KristenJestin/poste/releases/download/latest";
/// Nombre de versions gardées (celle de `current` en plus si elle est plus ancienne).
pub const KEEP: usize = 3;

/// Une version installée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub dir: PathBuf,
    pub commit: String,
    /// Date de construction (RFC 3339, UTC), pour l'ordre.
    pub date: String,
}

impl Version {
    /// Lit `VERSION` dans `dir` ; `None` si absent ou invalide.
    pub fn read(dir: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(dir.join(VERSION_FILE)).ok()?;
        let (mut commit, mut date) = (None, String::new());
        for l in text.lines() {
            match l.split_once('=') {
                Some(("commit", v)) => commit = Some(v.trim().to_string()),
                Some(("date", v)) => date = v.trim().to_string(),
                _ => {}
            }
        }
        let commit = commit.filter(|c| valid_commit(c))?;
        Some(Self {
            dir: dir.to_path_buf(),
            commit,
            date,
        })
    }

    pub fn short(&self) -> &str {
        &self.commit[..self.commit.len().min(7)]
    }
}

fn valid_commit(c: &str) -> bool {
    (7..=40).contains(&c.len()) && c.chars().all(|c| c.is_ascii_hexdigit())
}

/// Versions installées, la plus récente d'abord (date de construction, puis commit).
pub fn installed(env: &Env) -> Vec<Version> {
    let mut out: Vec<Version> = std::fs::read_dir(env.versions_dir())
        .map(|rd| {
            rd.flatten()
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .filter_map(|e| Version::read(&e.path()))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.commit.cmp(&a.commit)));
    out
}

/// Chemin réel de ce que désigne `current`, s'il existe.
pub fn current_target(env: &Env) -> Option<PathBuf> {
    std::fs::canonicalize(env.current_link()).ok()
}

/// `root` est-il une version installée (et non le clone) ?
pub fn is_release(env: &Env, root: &Path) -> bool {
    match (
        std::fs::canonicalize(root),
        std::fs::canonicalize(env.versions_dir()),
    ) {
        (Ok(r), Ok(v)) => r.starts_with(v),
        _ => false,
    }
}

/// La version installée qu'est `root`, si c'en est une.
pub fn version_of(env: &Env, root: &Path) -> Option<Version> {
    if !is_release(env, root) {
        return None;
    }
    Version::read(&std::fs::canonicalize(root).ok()?)
}

/// Refuse une écriture dans le dépôt quand la racine est une version installée (lecture seule).
pub fn ensure_writable(env: &Env, root: &Path, what: &str) -> Result<()> {
    if is_release(env, root) {
        bail!(
            "{what} écrit dans le dépôt, or poste tourne depuis une version installée (lecture seule) : passez en mode dev (`poste sys use dev`) puis relancez"
        );
    }
    Ok(())
}

/// Racine dont tout est lu : `current` s'il désigne quelque chose ; sinon, en mode dev, le
/// clone (machine pas encore passée à `current` : le prochain `setup` le pose).
pub fn root(env: &Env, cfg: &Config) -> Result<PathBuf> {
    let cur = env.current_link();
    if cur.exists() {
        return Ok(cur);
    }
    match cfg.source {
        Source::Dev => util::clone(env, cfg),
        Source::Release => bail!(
            "aucune version en place ({} absent ou cassé) : lancez `poste sys sync`",
            env.display(&cur)
        ),
    }
}

/// Simulation : la racine lue n'est pas `current` (pas encore posé ou pas encore basculé) ;
/// `current` est alors la racine où les liens de config pointeront pour de vrai.
pub fn link_root(env: &Env, root: &Path) -> Option<PathBuf> {
    (root != env.current_link()).then(|| env.current_link())
}

/// Fait pointer `current` sur `target` (remplacement atomique). Refuse si `current` existe sans
/// être un lien : poste ne l'a pas posé.
pub fn point_current(env: &Env, target: &Path) -> Result<()> {
    let cur = env.current_link();
    if let Ok(md) = std::fs::symlink_metadata(&cur) {
        if !md.file_type().is_symlink() {
            bail!(
                "{} existe et n'est pas un lien : déplacez-le vous-même, puis relancez",
                env.display(&cur)
            );
        }
    }
    util::replace_with_symlink(&cur, target)?;
    println!("Racine : {} -> {}", env.display(&cur), env.display(target));
    Ok(())
}

/// Pour `setup` : `current` s'il désigne quelque chose ; sinon il est posé (dev : le clone ;
/// release : la version installée la plus récente). `--dry-run` ne le pose pas et lit la cible.
pub fn ensure_current(env: &Env, cfg: &Config, dry_run: bool) -> Result<PathBuf> {
    let cur = env.current_link();
    if cur.exists() {
        return Ok(cur);
    }
    let target = match cfg.source {
        Source::Dev => util::clone(env, cfg)?,
        Source::Release => installed(env)
            .into_iter()
            .next()
            .map(|v| v.dir)
            .context("aucune version installée : lancez `poste sys sync`")?,
    };
    if dry_run {
        println!(
            "Simulation : lien {} -> {} non posé.",
            env.display(&cur),
            env.display(&target)
        );
        return Ok(target);
    }
    point_current(env, &target)?;
    Ok(cur)
}

/// En mode dev : `current` doit désigner le clone. Renvoie la racine à lire.
pub fn dev_current(env: &Env, cfg: &Config, dry_run: bool) -> Result<PathBuf> {
    let clone = util::clone(env, cfg)?;
    if current_target(env) == std::fs::canonicalize(&clone).ok() {
        return Ok(env.current_link());
    }
    if dry_run {
        println!(
            "Simulation : lien {} -> {} non posé.",
            env.display(&env.current_link()),
            env.display(&clone)
        );
        return Ok(clone);
    }
    point_current(env, &clone)?;
    Ok(env.current_link())
}

// ---------------------------------------------------------------- téléchargement

/// Téléchargement par `curl` (faux programme en test : `POSTE_SYS_TEST_CURL`).
pub struct Fetcher {
    program: PathBuf,
    /// Arguments placés avant ceux de `curl` (en test : le script lancé par `sh`).
    pre: Vec<PathBuf>,
    base: String,
}

impl Fetcher {
    /// `curl` et l'adresse publiée ; `POSTE_SYS_RELEASE_URL` remplace l'adresse (fork, essai).
    pub fn from_env() -> Self {
        Self {
            program: std::env::var_os("POSTE_SYS_TEST_CURL")
                .map(PathBuf::from)
                .unwrap_or_else(|| "curl".into()),
            pre: Vec::new(),
            base: std::env::var("POSTE_SYS_RELEASE_URL").unwrap_or_else(|_| DEFAULT_URL.into()),
        }
    }

    /// Faux `curl` en test : un script lancé par `sh` (jamais exécuté directement : un fichier
    /// tout juste écrit peut être « occupé » le temps qu'un autre test lance un processus).
    #[cfg(test)]
    pub fn with(script: &Path, base: &str) -> Self {
        Self {
            program: "sh".into(),
            pre: vec![script.to_path_buf()],
            base: base.into(),
        }
    }

    fn url(&self, name: &str) -> String {
        format!("{}/{name}", self.base.trim_end_matches('/'))
    }

    /// Contenu d'un petit fichier publié (la somme de contrôle), sans rien écrire.
    fn text(&self, name: &str) -> Result<String> {
        let url = self.url(name);
        let out = Command::new(&self.program)
            .args(&self.pre)
            .args(["-fsSL", "--retry", "2"])
            .arg(&url)
            .output()
            .with_context(|| format!("impossible de lancer {}", self.program.display()))?;
        if !out.status.success() {
            bail!("téléchargement de {url} échoué");
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn file(&self, name: &str, dest: &Path) -> Result<()> {
        let url = self.url(name);
        let st = Command::new(&self.program)
            .args(&self.pre)
            .args(["-fsSL", "--retry", "2", "-o"])
            .arg(dest)
            .arg(&url)
            .status()
            .with_context(|| format!("impossible de lancer {}", self.program.display()))?;
        if !st.success() {
            bail!("téléchargement de {url} échoué");
        }
        Ok(())
    }
}

/// Somme sha256 d'un fichier `.sha256` (format de `sha256sum` : `<hex>  <nom>`).
pub fn parse_sum(text: &str) -> Result<String> {
    let hex = text.split_whitespace().next().unwrap_or("").to_lowercase();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("somme de contrôle illisible : « {} »", text.trim());
    }
    Ok(hex)
}

fn sum_of(v: &Version) -> Option<String> {
    std::fs::read_to_string(v.dir.join(SUM_FILE))
        .ok()
        .map(|s| s.trim().to_string())
}

/// La dernière version publiée, installée si besoin : somme lue d'abord (rien à télécharger si
/// une version installée vient de la même archive), puis archive téléchargée, vérifiée et
/// extraite dans `versions/<commit>/`. Ne touche pas à `current`. `None` en `--dry-run` quand
/// il faudrait télécharger.
pub fn fetch(env: &Env, f: &Fetcher, dry_run: bool) -> Result<Option<Version>> {
    let sum = parse_sum(&f.text(&format!("{ARCHIVE}.sha256"))?)?;
    if let Some(v) = installed(env)
        .into_iter()
        .find(|v| sum_of(v).as_deref() == Some(sum.as_str()))
    {
        println!("Dernière version déjà installée : {}.", v.short());
        return Ok(Some(v));
    }
    if dry_run {
        println!("Simulation : nouvelle version publiée, téléchargement non lancé.");
        return Ok(None);
    }
    let cache = env.cache_dir().join("download");
    std::fs::create_dir_all(&cache)?;
    let archive = cache.join(ARCHIVE);
    println!("Téléchargement de {}…", f.url(ARCHIVE));
    f.file(ARCHIVE, &archive)?;
    let got = sha256_file(&archive)?;
    if got != sum {
        let _ = std::fs::remove_file(&archive);
        bail!(
            "somme de contrôle différente (attendue {}, obtenue {}) : archive refusée, rien n'est installé",
            util::short(&sum),
            util::short(&got)
        );
    }
    let v = extract(env, &archive, &sum);
    let _ = std::fs::remove_file(&archive);
    let v = v?;
    println!("Version {} installée ({}).", v.short(), env.display(&v.dir));
    Ok(Some(v))
}

/// Extrait une archive vérifiée dans `versions/<commit>/` (dossier temporaire puis renommage).
fn extract(env: &Env, archive: &Path, sum: &str) -> Result<Version> {
    let vdir = env.versions_dir();
    std::fs::create_dir_all(&vdir)?;
    let staging = vdir.join(format!(".extract-{}", std::process::id()));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    std::fs::create_dir_all(&staging)?;
    let res = (|| {
        let st = Command::new("tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(&staging)
            .arg("--no-same-owner")
            .status()
            .context("impossible de lancer tar")?;
        if !st.success() {
            bail!("extraction de l'archive échouée");
        }
        let v = Version::read(&staging).context("archive sans fichier VERSION valide")?;
        std::fs::write(staging.join(SUM_FILE), format!("{sum}\n"))?;
        let dest = vdir.join(&v.commit);
        if dest.exists() {
            // Même commit déjà là (archive reconstruite) : on garde l'installée, mais elle
            // retient la nouvelle somme, sinon l'archive serait téléchargée à chaque sync.
            std::fs::remove_dir_all(&staging)?;
            let tmp = dest.join(format!(".{SUM_FILE}-{}", std::process::id()));
            std::fs::write(&tmp, format!("{sum}\n"))?;
            std::fs::rename(&tmp, dest.join(SUM_FILE))?;
        } else {
            std::fs::rename(&staging, &dest)?;
        }
        Version::read(&dest).context("version installée illisible")
    })();
    if res.is_err() && staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    res
}

/// Retire les versions au-delà des `keep` plus récentes, jamais celle de `current`, et les
/// restes d'extraction interrompue (`.extract-<pid>` dont le processus n'existe plus : celui
/// d'un `sync` en cours n'est jamais touché). Renvoie les versions retirées.
pub fn prune(env: &Env, keep: usize) -> Result<Vec<String>> {
    let cur = current_target(env);
    let mut removed = Vec::new();
    for v in installed(env).iter().skip(keep) {
        if std::fs::canonicalize(&v.dir).ok() == cur {
            continue;
        }
        std::fs::remove_dir_all(&v.dir)
            .with_context(|| format!("retrait de {}", v.dir.display()))?;
        removed.push(v.short().to_string());
    }
    if let Ok(rd) = std::fs::read_dir(env.versions_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(pid) = name.strip_prefix(".extract-") else {
                continue;
            };
            // Un nom qui n'est pas un numéro de processus n'est pas de nous : laissé.
            if pid.parse::<u32>().is_ok() && !Path::new("/proc").join(pid).exists() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    Ok(removed)
}

// ---------------------------------------------------------------- écarts entre racines

/// Empreinte de `config/` et `modules/` d'une racine (fichier : sha256 ; lien : sa cible).
fn snapshot(root: &Path) -> Result<BTreeMap<String, String>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> Result<()> {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return Ok(());
        };
        for e in rd.flatten() {
            let name = e.file_name();
            if name == "target" || name == "__pycache__" {
                continue;
            }
            let p = e.path();
            let rel = p
                .strip_prefix(base)
                .expect("sous la racine")
                .to_string_lossy()
                .into_owned();
            let ft = e.file_type()?;
            if ft.is_symlink() {
                let t = std::fs::read_link(&p)?;
                out.insert(rel, format!("lien:{}", t.display()));
            } else if ft.is_dir() {
                walk(base, &p, out)?;
            } else {
                out.insert(rel, sha256_file(&p)?);
            }
        }
        Ok(())
    }
    let mut out = BTreeMap::new();
    for sub in ["config", "modules", "addons"] {
        walk(root, &root.join(sub), &mut out)?;
    }
    Ok(out)
}

/// Chemins (relatifs à la racine) qui diffèrent entre deux racines : ajoutés, retirés, changés.
pub fn changed_paths(old: &Path, new: &Path) -> Result<BTreeSet<String>> {
    if std::fs::canonicalize(old).ok() == std::fs::canonicalize(new).ok() {
        return Ok(BTreeSet::new());
    }
    let (a, b) = (snapshot(old)?, snapshot(new)?);
    Ok(a.keys()
        .chain(b.keys())
        .filter(|k| a.get(*k) != b.get(*k))
        .cloned()
        .collect())
}

/// Ajoute à `need` ce que demande le passage de `old` à `new` (rien sans ancienne racine connue :
/// tout ce que la nouvelle porte compte alors).
pub fn add_root_changes(need: &mut Need, old: Option<&Path>, new: &Path) -> Result<()> {
    let paths = match old {
        Some(o) => changed_paths(o, new)?,
        None => snapshot(new)?.into_keys().collect(),
    };
    for p in &paths {
        need.add_path(p);
    }
    Ok(())
}

// ---------------------------------------------------------------- poste sys versions / use

/// `poste sys versions` : la source, la racine et les versions installées.
pub fn list(env: &Env, cfg: &Config) -> Result<()> {
    println!("Source        {}", cfg.source.as_str());
    match std::fs::read_link(env.current_link()) {
        Ok(t) => println!(
            "Racine        {} -> {}{}",
            env.display(&env.current_link()),
            env.display(&t),
            if env.current_link().exists() {
                ""
            } else {
                "  (cassé)"
            }
        ),
        Err(_) => println!(
            "Racine        {} absent (le prochain `poste sys setup` le pose)",
            env.display(&env.current_link())
        ),
    }
    let cur = current_target(env);
    let all = installed(env);
    if all.is_empty() {
        println!(
            "\nAucune version installée (`poste sys use release` ou `poste sys sync` en release)."
        );
        return Ok(());
    }
    println!(
        "\nVersions installées ({}) :",
        env.display(&env.versions_dir())
    );
    for v in &all {
        let mark = if std::fs::canonicalize(&v.dir).ok() == cur {
            "*"
        } else {
            " "
        };
        println!("  {mark} {}  {}", v.short(), v.date);
    }
    Ok(())
}

/// Ce que demande `poste sys use <cible>`.
pub enum Target {
    Dev,
    /// La plus récente installée (téléchargée si aucune).
    Release,
    /// Une version installée, par début de commit (retour arrière).
    Pinned(String),
}

impl Target {
    pub fn parse(s: &str) -> Self {
        match s {
            "dev" => Target::Dev,
            "release" => Target::Release,
            other => Target::Pinned(other.to_string()),
        }
    }
}

/// Ce que `switch` a fait : code de sortie et ce qu'il faut recharger.
pub struct Switched {
    pub code: ExitCode,
    pub need: Need,
}

/// `poste sys use` : écrit `source`, fait pointer `current`, lance `setup` depuis la nouvelle
/// racine. Le rechargement (île, Hyprland) est laissé à l'appelant, d'après `need`.
pub fn use_target(
    env: &Env,
    cfg: &Config,
    target: &Target,
    f: &Fetcher,
    o: &setup::Opts,
) -> Result<Switched> {
    let (source, dir) = match target {
        Target::Dev => (Source::Dev, util::clone(env, cfg)?),
        Target::Release => match installed(env).into_iter().next() {
            Some(v) => (Source::Release, v.dir),
            None => match fetch(env, f, o.dry_run)? {
                Some(v) => (Source::Release, v.dir),
                None => {
                    return Ok(Switched {
                        code: ExitCode::SUCCESS,
                        need: Need::default(),
                    })
                }
            },
        },
        Target::Pinned(p) => {
            let m: Vec<Version> = installed(env)
                .into_iter()
                .filter(|v| v.commit.starts_with(p.as_str()))
                .collect();
            match m.as_slice() {
                [v] => (Source::Release, v.dir.clone()),
                [] => bail!(
                    "« {p} » : ni dev, ni release, ni le début d'une version installée (poste sys versions)"
                ),
                _ => bail!("« {p} » désigne plusieurs versions : donnez plus de caractères"),
            }
        }
    };
    switch(env, source, &dir, o)
}

/// Bascule la machine sur `dir` (source `source`) puis lance `setup` depuis `current`.
pub fn switch(env: &Env, source: Source, dir: &Path, o: &setup::Opts) -> Result<Switched> {
    let old = current_target(env);
    let mut need = Need::default();
    add_root_changes(&mut need, old.as_deref(), dir)?;
    if o.dry_run {
        println!(
            "Simulation : source = \"{}\" et {} -> {} non écrits, setup non lancé.",
            source.as_str(),
            env.display(&env.current_link()),
            env.display(dir)
        );
        return Ok(Switched {
            code: ExitCode::SUCCESS,
            need,
        });
    }
    let file = env.config_file();
    if machine::set_key(&file, "source", source.as_str())? {
        println!(
            "Config : source = \"{}\" ({}).",
            source.as_str(),
            env.display(&file)
        );
    }
    point_current(env, dir)?;
    let done = setup::run(env, &env.current_link(), o)?;
    if done.config_changed {
        need.shell = true;
        need.hypr = true;
    }
    Ok(Switched {
        code: done.code,
        need,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    /// Une version dans `dir` : `VERSION`, une config, un module avec une île.
    pub fn make_root(dir: &Path, commit: &str, date: &str, island: &str) {
        fs::create_dir_all(dir.join("config/.config/a")).unwrap();
        fs::write(dir.join("config/.config/a/neuf"), "n").unwrap();
        fs::create_dir_all(dir.join("modules/rec/island")).unwrap();
        fs::write(dir.join("modules/rec/module.toml"), "name = \"rec\"\n").unwrap();
        fs::write(dir.join("modules/rec/poste-rec"), "#!/bin/sh\n").unwrap();
        fs::write(dir.join("modules/rec/island/launcher.json"), island).unwrap();
        fs::create_dir_all(dir.join("target/release")).unwrap();
        fs::write(dir.join("target/release/poste"), format!("bin {commit}")).unwrap();
        if !commit.is_empty() {
            fs::write(
                dir.join(VERSION_FILE),
                format!("commit={commit}\ndate={date}\n"),
            )
            .unwrap();
        }
    }

    /// Archive `tar.gz` d'une racine, et son fichier `.sha256`, dans `pubdir`.
    pub fn publish(src: &Path, pubdir: &Path) -> String {
        fs::create_dir_all(pubdir).unwrap();
        let archive = pubdir.join(ARCHIVE);
        let st = Command::new("tar")
            .arg("-czf")
            .arg(&archive)
            .arg("-C")
            .arg(src)
            .arg(".")
            .status()
            .unwrap();
        assert!(st.success());
        let sum = sha256_file(&archive).unwrap();
        fs::write(
            pubdir.join(format!("{ARCHIVE}.sha256")),
            format!("{sum}  {ARCHIVE}\n"),
        )
        .unwrap();
        sum
    }

    /// Faux `curl` : copie `<pubdir>/<nom>` (dernier argument : l'adresse), vers `-o` ou la
    /// sortie ; chaque appel est noté dans `log`.
    pub fn fake_curl(dir: &Path, pubdir: &Path) -> (PathBuf, PathBuf) {
        let prog = dir.join("fake-curl");
        let log = dir.join("curl.log");
        fs::write(
            &prog,
            format!(
                "#!/bin/sh\nout=\nwhile [ $# -gt 1 ]; do\n  if [ \"$1\" = -o ]; then out=\"$2\"; shift; fi\n  shift\ndone\nname=$(basename \"$1\")\necho \"$name\" >> '{}'\n[ -f '{}'/\"$name\" ] || exit 22\nif [ -n \"$out\" ]; then cp '{}'/\"$name\" \"$out\"; else cat '{}'/\"$name\"; fi\n",
                log.display(),
                pubdir.display(),
                pubdir.display(),
                pubdir.display()
            ),
        )
        .unwrap();
        fs::set_permissions(&prog, fs::Permissions::from_mode(0o755)).unwrap();
        (prog, log)
    }

    #[test]
    fn lecture_de_version() {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join(VERSION_FILE),
            "commit=0123456789abcdef\ndate=2026-10-01T10:00:00Z\n",
        )
        .unwrap();
        let v = Version::read(d.path()).unwrap();
        assert_eq!(v.short(), "0123456");
        assert_eq!(v.date, "2026-10-01T10:00:00Z");
        fs::write(d.path().join(VERSION_FILE), "commit=../x\n").unwrap();
        assert!(Version::read(d.path()).is_none());
        assert!(parse_sum("abc").is_err());
        assert_eq!(
            parse_sum(&format!("{}  f\n", "A".repeat(64))).unwrap(),
            "a".repeat(64)
        );
    }

    #[test]
    fn telechargement_verifie_puis_deja_installe() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::with_home(d.path().join("home"));
        let src = d.path().join("src");
        make_root(&src, "aaaaaaa1", "2026-10-01T10:00:00Z", "{}");
        let pubdir = d.path().join("pub");
        let sum = publish(&src, &pubdir);
        let (curl, log) = fake_curl(d.path(), &pubdir);
        let f = Fetcher::with(&curl, "https://example.com/r");

        // Simulation : rien d'écrit.
        assert!(fetch(&env, &f, true).unwrap().is_none());
        assert!(!env.data_home.exists() && !env.cache_home.exists());

        let v = fetch(&env, &f, false).unwrap().unwrap();
        assert_eq!(v.dir, env.versions_dir().join("aaaaaaa1"));
        assert!(v.dir.join("config/.config/a/neuf").is_file());
        assert_eq!(
            fs::read_to_string(v.dir.join(SUM_FILE)).unwrap().trim(),
            sum
        );
        assert!(!env.cache_dir().join("download").join(ARCHIVE).exists());
        // Deuxième fois : seule la somme est lue.
        fs::write(&log, "").unwrap();
        assert_eq!(fetch(&env, &f, false).unwrap().unwrap().commit, "aaaaaaa1");
        assert_eq!(
            fs::read_to_string(&log).unwrap().trim(),
            format!("{ARCHIVE}.sha256")
        );
    }

    #[test]
    fn archive_reconstruite_du_meme_commit_retient_la_nouvelle_somme() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::with_home(d.path().join("home"));
        let src = d.path().join("src");
        make_root(&src, "aaaaaaa1", "2026-10-01T10:00:00Z", "{}");
        let pubdir = d.path().join("pub");
        let first = publish(&src, &pubdir);
        let (curl, log) = fake_curl(d.path(), &pubdir);
        let f = Fetcher::with(&curl, "https://example.com/r");
        fetch(&env, &f, false).unwrap().unwrap();
        // Même commit, archive refaite (contenu différent donc somme différente).
        fs::write(src.join("config/.config/a/neuf"), "refait").unwrap();
        let second = publish(&src, &pubdir);
        assert_ne!(first, second);
        let v = fetch(&env, &f, false).unwrap().unwrap();
        assert_eq!(v.commit, "aaaaaaa1");
        assert_eq!(
            fs::read_to_string(v.dir.join(SUM_FILE)).unwrap().trim(),
            second
        );
        assert_eq!(installed(&env).len(), 1);
        // Le sync suivant ne télécharge plus l'archive : seule la somme est lue.
        fs::write(&log, "").unwrap();
        fetch(&env, &f, false).unwrap().unwrap();
        assert_eq!(
            fs::read_to_string(&log).unwrap().trim(),
            format!("{ARCHIVE}.sha256")
        );
        // Aucun fichier temporaire ne reste dans la version.
        assert!(fs::read_dir(&v.dir)
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().starts_with('.')));
    }

    #[test]
    fn somme_fausse_refusee_sans_rien_installer() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::with_home(d.path().join("home"));
        let src = d.path().join("src");
        make_root(&src, "aaaaaaa1", "2026-10-01T10:00:00Z", "{}");
        let pubdir = d.path().join("pub");
        publish(&src, &pubdir);
        fs::write(
            pubdir.join(format!("{ARCHIVE}.sha256")),
            format!("{}  {ARCHIVE}\n", "0".repeat(64)),
        )
        .unwrap();
        let (curl, _) = fake_curl(d.path(), &pubdir);
        let f = Fetcher::with(&curl, "https://example.com/r");
        let e = fetch(&env, &f, false).unwrap_err();
        assert!(format!("{e:#}").contains("somme de contrôle"));
        assert!(installed(&env).is_empty());
        assert!(!env.cache_dir().join("download").join(ARCHIVE).exists());
        // Sans VERSION : refusée aussi, aucun reste.
        let src2 = d.path().join("src2");
        make_root(&src2, "", "", "{}");
        publish(&src2, &pubdir);
        assert!(fetch(&env, &f, false).is_err());
        assert!(fs::read_dir(env.versions_dir()).unwrap().next().is_none());
    }

    #[test]
    fn elagage_garde_les_plus_recentes_et_current() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::with_home(d.path().join("home"));
        let vd = env.versions_dir();
        for (i, c) in ["1000000", "2000000", "3000000", "4000000", "5000000"]
            .iter()
            .enumerate()
        {
            make_root(
                &vd.join(c),
                c,
                &format!("2026-10-0{}T10:00:00Z", i + 1),
                "{}",
            );
        }
        // Reste d'un processus disparu : retiré. Celui d'un sync en cours (le nôtre) : laissé.
        let mut done = Command::new("true").spawn().unwrap();
        let dead = done.id();
        done.wait().unwrap();
        let (dead_dir, alive_dir) = (
            vd.join(format!(".extract-{dead}")),
            vd.join(format!(".extract-{}", std::process::id())),
        );
        fs::create_dir_all(&dead_dir).unwrap();
        fs::create_dir_all(&alive_dir).unwrap();
        // current sur la plus ancienne (retour arrière) : jamais retirée.
        point_current(&env, &vd.join("1000000")).unwrap();
        let removed = prune(&env, 3).unwrap();
        assert_eq!(removed, ["2000000"]);
        let left: Vec<String> = installed(&env).into_iter().map(|v| v.commit).collect();
        assert_eq!(left, ["5000000", "4000000", "3000000", "1000000"]);
        assert!(!dead_dir.exists());
        assert!(alive_dir.exists());
        assert!(env.current_link().exists());
    }

    #[test]
    fn current_jamais_pose_sur_un_fichier_ordinaire() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::with_home(d.path().join("home"));
        fs::create_dir_all(env.current_link()).unwrap();
        assert!(point_current(&env, d.path()).is_err());
        assert!(env.current_link().is_dir() && !env.current_link().is_symlink());
    }

    #[test]
    fn ecarts_entre_deux_racines() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        make_root(&a, "aaaaaaa1", "x", "{}");
        make_root(&b, "aaaaaaa2", "y", "{}");
        // Seul le binaire et VERSION diffèrent : rien à recharger.
        let mut n = Need::default();
        add_root_changes(&mut n, Some(&a), &b).unwrap();
        assert!(!n.any());
        fs::write(b.join("modules/rec/island/launcher.json"), "{\"x\":1}").unwrap();
        fs::create_dir_all(b.join("config/.config/hypr")).unwrap();
        fs::write(b.join("config/.config/hypr/h.lua"), "--").unwrap();
        let ch = changed_paths(&a, &b).unwrap();
        assert_eq!(
            ch.into_iter().collect::<Vec<_>>(),
            [
                "config/.config/hypr/h.lua",
                "modules/rec/island/launcher.json"
            ]
        );
        let mut n = Need::default();
        add_root_changes(&mut n, Some(&a), &b).unwrap();
        assert!(n.shell && n.hypr);
        // Même racine : rien.
        assert!(changed_paths(&b, &b).unwrap().is_empty());
    }
}
