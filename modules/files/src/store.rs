//! The index on disk, `$XDG_CACHE_HOME/poste/files.idx`: rebuilt by a full walk (`refresh`,
//! started in the background by SUPER+D), read again at each search. Nothing stays in memory
//! between two commands. The file is replaced in one go (written beside, then renamed): a
//! search reads the old one or the new one, never a piece.

use crate::config::Config;
use crate::index::{self, Index, Meta};
use crate::rules::{walk, Rules};
use anyhow::{Context, Result};
use poste_core::Env;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{BufRead, BufReader};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

/// Walk threads (the disk is the limit; leave room for the rest).
const WALK_THREADS: usize = 4;
/// An index newer than this is not rebuilt (SUPER+D several times in a row).
const FRESH: Duration = Duration::from_secs(10);
/// Age past which a search starts a walk in the background (launcher opened by a keyword,
/// without SUPER+D): the search answers at once with the index in place.
const STALE: Duration = Duration::from_secs(60);

pub fn index_path(env: &Env) -> PathBuf {
    env.cache_dir().join("files.idx")
}

fn lock_path(env: &Env) -> PathBuf {
    env.cache_dir().join("files.lock")
}

pub fn log_path(env: &Env) -> PathBuf {
    env.state_dir().join("files.log")
}

/// Lock of the walks: one at a time.
fn open_lock(env: &Env) -> Result<File> {
    std::fs::create_dir_all(env.cache_dir())?;
    let p = lock_path(env);
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&p)
        .with_context(|| format!("opening {}", p.display()))
}

/// Age of the index and its header, without reading the rest; `None` when there is none.
pub fn peek(env: &Env) -> Option<(Duration, Meta)> {
    let f = File::open(index_path(env)).ok()?;
    let age = f
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .unwrap_or_default();
    let mut head = String::new();
    BufReader::new(f).read_line(&mut head).ok()?;
    Some((age, Meta::parse(head.trim_end())?))
}

/// What `refresh` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Refresh {
    Done(Meta),
    /// Another walk was running, or the index had just been rebuilt.
    Skipped,
}

/// Walks the roots and replaces the index. `force`: even when it is recent.
pub fn refresh(env: &Env, force: bool) -> Result<Refresh> {
    let lock = open_lock(env)?;
    match lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Ok(Refresh::Skipped),
        Err(TryLockError::Error(e)) => return Err(e).context("index lock"),
    }
    if !force && peek(env).is_some_and(|(age, _)| age < FRESH) {
        return Ok(Refresh::Skipped);
    }
    scan(env).map(Refresh::Done)
}

/// The walk itself; the caller holds the lock.
fn scan(env: &Env) -> Result<Meta> {
    let cfg = Config::load(env)?;
    let roots = cfg.root_paths(env)?;
    let user = Config::load_ignore(env)?;
    let rules = Arc::new(Rules::new(&roots, &user, "files.ignore")?);
    let start = Instant::now();
    let mut entries = Vec::new();
    for root in &roots {
        if root.is_dir() {
            entries.extend(walk(&rules, root, WALK_THREADS));
        } else {
            eprintln!("poste files: missing root: {}", root.display());
        }
    }
    let scan_ms = start.elapsed().as_millis() as u64;
    let (text, skipped) = index::serialize(&entries, &env.home, scan_ms);
    if skipped > 0 {
        eprintln!("poste files: {skipped} paths left out (not UTF-8 or with a line break)");
    }
    let dest = index_path(env);
    let tmp = dest.with_extension(format!("idx.{}", std::process::id()));
    std::fs::write(&tmp, &text).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, &dest).with_context(|| format!("replacing {}", dest.display()))?;
    Ok(Meta::parse(text.lines().next().unwrap_or_default()).expect("header written above"))
}

/// Loads the index; when there is none yet, waits for the running walk or does one.
pub fn load(env: &Env) -> Result<Index> {
    let path = index_path(env);
    if !path.exists() {
        let lock = open_lock(env)?;
        lock.lock().context("index lock")?;
        if !path.exists() {
            scan(env)?;
        }
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Index::parse(text, &env.home).with_context(|| path.display().to_string())
}

/// Starts `refresh` detached (own process group) and returns at once; its errors go to
/// `files.log`.
pub fn refresh_in_background(env: &Env) -> Result<()> {
    let log = log_path(env);
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let err = File::create(&log).with_context(|| format!("opening {}", log.display()))?;
    let exe = std::env::current_exe().context("program path")?;
    Command::new(exe)
        .arg("refresh")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(err)
        .process_group(0)
        .spawn()
        .context("starting the walk")?;
    Ok(())
}

/// Starts a walk in the background when the index is older than `STALE` and no walk is
/// running (not one process per keystroke during a walk).
pub fn refresh_if_stale(env: &Env) -> Result<()> {
    if !peek(env).is_some_and(|(age, _)| age > STALE) {
        return Ok(());
    }
    let lock = open_lock(env)?;
    match lock.try_lock() {
        Ok(()) => drop(lock),
        Err(_) => return Ok(()),
    }
    refresh_in_background(env)
}
