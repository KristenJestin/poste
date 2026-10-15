//! Installed versions and the poste root.
//!
//! Everything `setup` puts in place is read from a single root, the link `~/.local/share/poste/current`:
//! an installed version (`~/.local/share/poste/versions/<commit>/`, archive built by the CI)
//! or the clone of the repository (dev mode, key `repo`). A version has the layout of the repository:
//! `config/`, `modules/`, `bootstrap/`, the binaries in `target/release/`, plus `VERSION`
//! (`commit=…`, `date=…`); without `Cargo.toml`, `setup` builds nothing.

use crate::reload::Need;
use crate::util::{self, sha256_file};
use crate::{handoff, machine, setup};
use anyhow::{bail, Context, Result};
use poste_core::{Config, Env, Source};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Identity file of a version, at its root (written by the CI).
pub const VERSION_FILE: &str = "VERSION";
/// Checksum of the archive a version comes from (written at install time).
pub const SUM_FILE: &str = "ARCHIVE_SHA256";
/// Name of the published archive, and of its checksum (`<archive>.sha256`).
pub const ARCHIVE: &str = "poste-x86_64-linux.tar.gz";
/// Where the CI publishes the archive: a single `latest` prerelease, replaced on each push to `main`.
pub const DEFAULT_URL: &str = "https://github.com/KristenJestin/poste/releases/download/latest";
/// Number of versions kept (the one of `current` in addition if it is older).
pub const KEEP: usize = 3;

/// An installed version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub dir: PathBuf,
    pub commit: String,
    /// Build date (RFC 3339, UTC), for ordering.
    pub date: String,
}

impl Version {
    /// Reads `VERSION` in `dir`; `None` if absent or invalid.
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

/// Installed versions, most recent first (build date, then commit).
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

/// Real path of what `current` designates, if it exists.
pub fn current_target(env: &Env) -> Option<PathBuf> {
    std::fs::canonicalize(env.current_link()).ok()
}

/// Is `root` an installed version (and not the clone)?
pub fn is_release(env: &Env, root: &Path) -> bool {
    match (
        std::fs::canonicalize(root),
        std::fs::canonicalize(env.versions_dir()),
    ) {
        (Ok(r), Ok(v)) => r.starts_with(v),
        _ => false,
    }
}

/// The installed version that `root` is, if it is one.
pub fn version_of(env: &Env, root: &Path) -> Option<Version> {
    if !is_release(env, root) {
        return None;
    }
    Version::read(&std::fs::canonicalize(root).ok()?)
}

/// Refuses a write into the repository when the root is an installed version (read-only).
pub fn ensure_writable(env: &Env, root: &Path, what: &str) -> Result<()> {
    if is_release(env, root) {
        bail!(
            "{what} writes into the repository, but poste runs from an installed version (read-only): switch to dev mode (`poste sys use dev`) then run again"
        );
    }
    Ok(())
}

/// Root everything is read from: `current` if it designates something; otherwise, in dev mode, the
/// clone (machine not yet moved to `current`: the next `setup` puts it in place).
pub fn root(env: &Env, cfg: &Config) -> Result<PathBuf> {
    let cur = env.current_link();
    if cur.exists() {
        return Ok(cur);
    }
    match cfg.source {
        Source::Dev => util::clone(env, cfg),
        Source::Release => bail!(
            "no version in place ({} absent or broken): run `poste sys sync`",
            env.display(&cur)
        ),
    }
}

/// Dry run: the root read is not `current` (not put in place yet or not switched yet);
/// `current` is then the root the config links will really point to.
pub fn link_root(env: &Env, root: &Path) -> Option<PathBuf> {
    (root != env.current_link()).then(|| env.current_link())
}

/// Points `current` at `target` (atomic replacement). Refuses if `current` exists without
/// being a link: poste did not put it in place.
pub fn point_current(env: &Env, target: &Path) -> Result<()> {
    let cur = env.current_link();
    if let Ok(md) = std::fs::symlink_metadata(&cur) {
        if !md.file_type().is_symlink() {
            bail!(
                "{} exists and is not a link: move it yourself, then run again",
                env.display(&cur)
            );
        }
    }
    util::replace_with_symlink(&cur, target)?;
    println!("Root: {} -> {}", env.display(&cur), env.display(target));
    Ok(())
}

/// For `setup`: `current` if it designates something; otherwise it is put in place (dev: the clone;
/// release: the most recent installed version). `--dry-run` does not put it in place and reads the target.
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
            .context("no installed version: run `poste sys sync`")?,
    };
    if dry_run {
        println!(
            "Dry run: link {} -> {} not put in place.",
            env.display(&cur),
            env.display(&target)
        );
        return Ok(target);
    }
    point_current(env, &target)?;
    Ok(cur)
}

/// In dev mode: `current` must designate the clone. Returns the root to read.
pub fn dev_current(env: &Env, cfg: &Config, dry_run: bool) -> Result<PathBuf> {
    let clone = util::clone(env, cfg)?;
    if current_target(env) == std::fs::canonicalize(&clone).ok() {
        return Ok(env.current_link());
    }
    if dry_run {
        println!(
            "Dry run: link {} -> {} not put in place.",
            env.display(&env.current_link()),
            env.display(&clone)
        );
        return Ok(clone);
    }
    point_current(env, &clone)?;
    Ok(env.current_link())
}

// ---------------------------------------------------------------- download

/// Download through `curl` (fake program in tests: `POSTE_SYS_TEST_CURL`).
pub struct Fetcher {
    program: PathBuf,
    /// Arguments placed before those of `curl` (in tests: the script run by `sh`).
    pre: Vec<PathBuf>,
    base: String,
}

impl Fetcher {
    /// `curl` and the published address; `POSTE_SYS_RELEASE_URL` replaces the address (fork, trial).
    pub fn from_env() -> Self {
        Self {
            program: std::env::var_os("POSTE_SYS_TEST_CURL")
                .map(PathBuf::from)
                .unwrap_or_else(|| "curl".into()),
            pre: Vec::new(),
            base: std::env::var("POSTE_SYS_RELEASE_URL").unwrap_or_else(|_| DEFAULT_URL.into()),
        }
    }

    /// Fake `curl` in tests: a script run by `sh` (never executed directly: a file
    /// that was just written can be "busy" while another test starts a process).
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

    /// Content of a small published file (the checksum), without writing anything.
    fn text(&self, name: &str) -> Result<String> {
        let url = self.url(name);
        let out = Command::new(&self.program)
            .args(&self.pre)
            .args(["-fsSL", "--retry", "2"])
            .arg(&url)
            .output()
            .with_context(|| format!("cannot run {}", self.program.display()))?;
        if !out.status.success() {
            bail!("download of {url} failed");
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
            .with_context(|| format!("cannot run {}", self.program.display()))?;
        if !st.success() {
            bail!("download of {url} failed");
        }
        Ok(())
    }
}

/// sha256 sum of a `.sha256` file (format of `sha256sum`: `<hex>  <name>`).
pub fn parse_sum(text: &str) -> Result<String> {
    let hex = text.split_whitespace().next().unwrap_or("").to_lowercase();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("unreadable checksum: '{}'", text.trim());
    }
    Ok(hex)
}

fn sum_of(v: &Version) -> Option<String> {
    std::fs::read_to_string(v.dir.join(SUM_FILE))
        .ok()
        .map(|s| s.trim().to_string())
}

/// The latest published version, installed if needed: checksum read first (nothing to download if
/// an installed version comes from the same archive), then archive downloaded, verified and
/// extracted into `versions/<commit>/`. Does not touch `current`. `None` in `--dry-run` when
/// a download would be needed.
pub fn fetch(env: &Env, f: &Fetcher, dry_run: bool) -> Result<Option<Version>> {
    let sum = parse_sum(&f.text(&format!("{ARCHIVE}.sha256"))?)?;
    if let Some(v) = installed(env)
        .into_iter()
        .find(|v| sum_of(v).as_deref() == Some(sum.as_str()))
    {
        println!("Latest version already installed: {}.", v.short());
        return Ok(Some(v));
    }
    if dry_run {
        println!("Dry run: new published version, download not run.");
        return Ok(None);
    }
    let cache = env.cache_dir().join("download");
    std::fs::create_dir_all(&cache)?;
    let archive = cache.join(ARCHIVE);
    println!("Downloading {}…", f.url(ARCHIVE));
    f.file(ARCHIVE, &archive)?;
    let got = sha256_file(&archive)?;
    if got != sum {
        let _ = std::fs::remove_file(&archive);
        bail!(
            "different checksum (expected {}, got {}): archive refused, nothing is installed",
            util::short(&sum),
            util::short(&got)
        );
    }
    let v = extract(env, &archive, &sum);
    let _ = std::fs::remove_file(&archive);
    let v = v?;
    println!("Version {} installed ({}).", v.short(), env.display(&v.dir));
    Ok(Some(v))
}

/// Extracts a verified archive into `versions/<commit>/` (temporary folder then rename).
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
            .context("cannot run tar")?;
        if !st.success() {
            bail!("extraction of the archive failed");
        }
        let v = Version::read(&staging).context("archive without a valid VERSION file")?;
        std::fs::write(staging.join(SUM_FILE), format!("{sum}\n"))?;
        let dest = vdir.join(&v.commit);
        if dest.exists() {
            // Same commit already there (rebuilt archive): we keep the installed one, but it
            // takes the new checksum, otherwise the archive would be downloaded on every sync.
            std::fs::remove_dir_all(&staging)?;
            let tmp = dest.join(format!(".{SUM_FILE}-{}", std::process::id()));
            std::fs::write(&tmp, format!("{sum}\n"))?;
            std::fs::rename(&tmp, dest.join(SUM_FILE))?;
        } else {
            std::fs::rename(&staging, &dest)?;
        }
        Version::read(&dest).context("installed version unreadable")
    })();
    if res.is_err() && staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    res
}

/// Removes the versions beyond the `keep` most recent, never the one of `current`, and the
/// leftovers of an interrupted extraction (`.extract-<pid>` whose process no longer exists: the one
/// of a running `sync` is never touched). Returns the versions removed.
pub fn prune(env: &Env, keep: usize) -> Result<Vec<String>> {
    let cur = current_target(env);
    let mut removed = Vec::new();
    for v in installed(env).iter().skip(keep) {
        if std::fs::canonicalize(&v.dir).ok() == cur {
            continue;
        }
        std::fs::remove_dir_all(&v.dir)
            .with_context(|| format!("cannot remove {}", v.dir.display()))?;
        removed.push(v.short().to_string());
    }
    if let Ok(rd) = std::fs::read_dir(env.versions_dir()) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(pid) = name.strip_prefix(".extract-") else {
                continue;
            };
            // A name that is not a process number is not ours: left.
            if pid.parse::<u32>().is_ok() && !Path::new("/proc").join(pid).exists() {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
    }
    Ok(removed)
}

// ---------------------------------------------------------------- differences between roots

/// Fingerprint of `config/` and `modules/` of a root (file: sha256; link: its target).
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
                .expect("under the root")
                .to_string_lossy()
                .into_owned();
            let ft = e.file_type()?;
            if ft.is_symlink() {
                let t = std::fs::read_link(&p)?;
                out.insert(rel, format!("link:{}", t.display()));
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

/// Paths (relative to the root) that differ between two roots: added, removed, changed.
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

/// Adds to `need` what the move from `old` to `new` asks for (nothing without a known old root:
/// everything the new one carries then counts).
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

/// `poste sys versions`: the source, the root and the installed versions.
pub fn list(env: &Env, cfg: &Config) -> Result<()> {
    println!("Source        {}", cfg.source.as_str());
    match std::fs::read_link(env.current_link()) {
        Ok(t) => println!(
            "Root          {} -> {}{}",
            env.display(&env.current_link()),
            env.display(&t),
            if env.current_link().exists() {
                ""
            } else {
                "  (broken)"
            }
        ),
        Err(_) => println!(
            "Root          {} absent (the next `poste sys setup` puts it in place)",
            env.display(&env.current_link())
        ),
    }
    let cur = current_target(env);
    let all = installed(env);
    if all.is_empty() {
        println!(
            "\nNo installed version (`poste sys use release` or `poste sys sync` in release)."
        );
        return Ok(());
    }
    println!(
        "\nInstalled versions ({}):",
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

/// What `poste sys use <target>` asks for.
pub enum Target {
    Dev,
    /// The most recent installed (downloaded if none).
    Release,
    /// An installed version, by start of commit (rollback).
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

/// What `switch` did: exit code and what must be reloaded.
pub struct Switched {
    pub code: ExitCode,
    pub need: Need,
}

/// `poste sys use`: writes `source`, points `current`, runs `setup` from the new
/// root. The reload (island, Hyprland) is left to the caller, based on `need`.
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
                    "'{p}': neither dev, nor release, nor the start of an installed version (poste sys versions)"
                ),
                _ => bail!("'{p}' designates several versions: give more characters"),
            }
        }
    };
    switch(env, source, &dir, o)
}

/// Switches the machine to `dir` (source `source`) then runs `setup` from `current`.
pub fn switch(env: &Env, source: Source, dir: &Path, o: &setup::Opts) -> Result<Switched> {
    let old = current_target(env);
    let mut need = Need::default();
    add_root_changes(&mut need, old.as_deref(), dir)?;
    if o.dry_run {
        println!(
            "Dry run: source = \"{}\" and {} -> {} not written, setup not run.",
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
            "Config: source = \"{}\" ({}).",
            source.as_str(),
            env.display(&file)
        );
    }
    point_current(env, dir)?;
    handoff::complete(env, old.as_deref(), need, o.yes, false, o.skip_build)
}

/// The rest of `sync` and `use` once `current` is set: `setup` from `root` (`current`,
/// or the root read in a dry run), pruning of the old versions if `prune_old`. Run here, or by the old `poste-sys` that hands over
/// (`poste-sys finish`, see `handoff`).
pub fn finish(
    env: &Env,
    root: &Path,
    mut need: Need,
    opts: &setup::Opts,
    prune_old: bool,
) -> Result<Switched> {
    let done = setup::run(env, root, opts)?;
    if done.config_changed {
        need.shell = true;
        need.hypr = true;
    }
    if prune_old {
        let removed = prune(env, KEEP)?;
        if !removed.is_empty() {
            println!("Old versions removed: {}.", removed.join(", "));
        }
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

    /// A version in `dir`: `VERSION`, a config, a module with an island.
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

    /// `tar.gz` archive of a root, and its `.sha256` file, in `pubdir`.
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

    /// Fake `curl`: copies `<pubdir>/<name>` (last argument: the address), to `-o` or the
    /// output; each call is noted in `log`.
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
    fn reading_a_version() {
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
    fn verified_download_then_already_installed() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::with_home(d.path().join("home"));
        let src = d.path().join("src");
        make_root(&src, "aaaaaaa1", "2026-10-01T10:00:00Z", "{}");
        let pubdir = d.path().join("pub");
        let sum = publish(&src, &pubdir);
        let (curl, log) = fake_curl(d.path(), &pubdir);
        let f = Fetcher::with(&curl, "https://example.com/r");

        // Dry run: nothing written.
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
        // Second time: only the checksum is read.
        fs::write(&log, "").unwrap();
        assert_eq!(fetch(&env, &f, false).unwrap().unwrap().commit, "aaaaaaa1");
        assert_eq!(
            fs::read_to_string(&log).unwrap().trim(),
            format!("{ARCHIVE}.sha256")
        );
    }

    #[test]
    fn rebuilt_archive_of_same_commit_keeps_the_new_checksum() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::with_home(d.path().join("home"));
        let src = d.path().join("src");
        make_root(&src, "aaaaaaa1", "2026-10-01T10:00:00Z", "{}");
        let pubdir = d.path().join("pub");
        let first = publish(&src, &pubdir);
        let (curl, log) = fake_curl(d.path(), &pubdir);
        let f = Fetcher::with(&curl, "https://example.com/r");
        fetch(&env, &f, false).unwrap().unwrap();
        // Same commit, archive rebuilt (different content therefore different checksum).
        fs::write(src.join("config/.config/a/neuf"), "redone").unwrap();
        let second = publish(&src, &pubdir);
        assert_ne!(first, second);
        let v = fetch(&env, &f, false).unwrap().unwrap();
        assert_eq!(v.commit, "aaaaaaa1");
        assert_eq!(
            fs::read_to_string(v.dir.join(SUM_FILE)).unwrap().trim(),
            second
        );
        assert_eq!(installed(&env).len(), 1);
        // The next sync no longer downloads the archive: only the checksum is read.
        fs::write(&log, "").unwrap();
        fetch(&env, &f, false).unwrap().unwrap();
        assert_eq!(
            fs::read_to_string(&log).unwrap().trim(),
            format!("{ARCHIVE}.sha256")
        );
        // No temporary file stays in the version.
        assert!(fs::read_dir(&v.dir)
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().starts_with('.')));
    }

    #[test]
    fn wrong_checksum_refused_without_installing_anything() {
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
        assert!(format!("{e:#}").contains("checksum"));
        assert!(installed(&env).is_empty());
        assert!(!env.cache_dir().join("download").join(ARCHIVE).exists());
        // Without VERSION: refused too, no leftover.
        let src2 = d.path().join("src2");
        make_root(&src2, "", "", "{}");
        publish(&src2, &pubdir);
        assert!(fetch(&env, &f, false).is_err());
        assert!(fs::read_dir(env.versions_dir()).unwrap().next().is_none());
    }

    #[test]
    fn pruning_keeps_the_most_recent_and_current() {
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
        // Leftover of a process that is gone: removed. The one of a running sync (ours): left.
        let mut done = Command::new("true").spawn().unwrap();
        let dead = done.id();
        done.wait().unwrap();
        let (dead_dir, alive_dir) = (
            vd.join(format!(".extract-{dead}")),
            vd.join(format!(".extract-{}", std::process::id())),
        );
        fs::create_dir_all(&dead_dir).unwrap();
        fs::create_dir_all(&alive_dir).unwrap();
        // current on the oldest (rollback): never removed.
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
    fn current_never_put_on_an_ordinary_file() {
        let d = tempfile::tempdir().unwrap();
        let env = Env::with_home(d.path().join("home"));
        fs::create_dir_all(env.current_link()).unwrap();
        assert!(point_current(&env, d.path()).is_err());
        assert!(env.current_link().is_dir() && !env.current_link().is_symlink());
    }

    #[test]
    fn differences_between_two_roots() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        make_root(&a, "aaaaaaa1", "x", "{}");
        make_root(&b, "aaaaaaa2", "y", "{}");
        // Only the binary and VERSION differ: nothing to reload.
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
        // Same root: nothing.
        assert!(changed_paths(&b, &b).unwrap().is_empty());
    }
}
