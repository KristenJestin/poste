//! Software installed outside the package repositories (`[[app]]` of `steps.toml`: GitHub
//! releases) and the third-party Omarchy plugins: what is installed, what is available, and the
//! update.
//!
//! No version is stored, neither in the repository nor on the machine: the installed one is read
//! on the system each time (`pacman -Q`, `<program> --version`, the plugin's git `HEAD`), the
//! available one from its source. An update made by hand or by `yay` is therefore always seen.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

// ------------------------------------------------------------------ data

/// A program published as a GitHub release asset. Exactly one way to install it:
/// `package` (a pacman package: `sudo pacman -U`, printed), `bin` (one file of the archive into
/// `~/.local/bin`) or `script` (the archive is extracted and this script of it is run).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub id: String,
    pub title: String,
    /// `owner/name` of the GitHub repository.
    pub github: String,
    /// Name of the asset; `{version}` is the release's version, `*` any text.
    pub asset: String,
    /// Follows the most recent release, pre-releases included (otherwise GitHub's "latest").
    #[serde(default)]
    pub prerelease: bool,
    pub package: Option<String>,
    /// Path of the program in the archive; `{version}` is the release's version.
    pub bin: Option<String>,
    pub script: Option<String>,
    /// Command whose output gives the installed version (last word of its first line); not
    /// needed with `package` (`pacman -Q`). Failing or absent program: not installed.
    #[serde(default)]
    pub version: Vec<String>,
    /// Command that succeeds while the program must not be replaced (music playing…).
    #[serde(default)]
    pub busy: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum Install<'a> {
    Package(&'a str),
    Bin(&'a str),
    Script(&'a str),
}

impl App {
    pub fn install(&self) -> Result<Install<'_>> {
        match (&self.package, &self.bin, &self.script) {
            (Some(p), None, None) => Ok(Install::Package(p)),
            (None, Some(_), None) | (None, None, Some(_)) if self.version.is_empty() => bail!(
                "{}: `version` is needed to know whether it is installed",
                self.id
            ),
            (None, Some(b), None) => Ok(Install::Bin(b)),
            (None, None, Some(s)) => Ok(Install::Script(s)),
            _ => bail!(
                "{}: exactly one of package, bin, script is expected",
                self.id
            ),
        }
    }
}

/// External programs, replaced by fakes in the tests (never the network there).
#[derive(Debug, Clone)]
pub struct Tools {
    pub gh: Vec<String>,
    pub pacman: Vec<String>,
    pub git: Vec<String>,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            gh: vec!["gh".into()],
            pacman: vec!["pacman".into()],
            git: vec!["git".into()],
        }
    }
}

/// Runs `prog` (program then its first arguments) with `args`; its trimmed output.
fn output(prog: &[String], args: &[&str]) -> Result<String> {
    let (p, pre) = prog.split_first().context("empty command")?;
    let o = Command::new(p)
        .args(pre)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("cannot run {p}"))?;
    if !o.status.success() {
        let err = String::from_utf8_lossy(&o.stderr);
        bail!("{p} {} failed: {}", args.join(" "), err.trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Runs a command without output; true if it ends with code 0.
pub(crate) fn succeeds(cmd: &[String]) -> bool {
    let Some((prog, args)) = cmd.split_first() else {
        return false;
    };
    Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

// ------------------------------------------------------------------ versions

/// Version of a tag or of a `--version` line: last word, without a leading `v`.
pub fn norm(s: &str) -> String {
    let w = s.split_whitespace().last().unwrap_or("");
    w.strip_prefix('v').unwrap_or(w).to_string()
}

/// `pacman -Q` version without its package release (`0.1.2-1` -> `0.1.2`).
fn strip_pkgrel(v: &str) -> &str {
    match v.rsplit_once('-') {
        Some((base, rel)) if !rel.is_empty() && rel.chars().all(|c| c.is_ascii_digit()) => base,
        _ => v,
    }
}

/// Installed version (`None`: not installed). `cmd` is the `version` command, substituted.
pub fn installed(app: &App, cmd: &[String], tools: &Tools) -> Option<String> {
    if let Some(p) = &app.package {
        let out = output(&tools.pacman, &["-Q", p]).ok()?;
        let v = out.split_whitespace().nth(1)?;
        return Some(strip_pkgrel(v).to_string());
    }
    let (prog, args) = cmd.split_first()?;
    let o = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !o.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&o.stdout);
    let v = norm(text.lines().next().unwrap_or(""));
    (!v.is_empty()).then_some(v)
}

/// Compares two versions: pacman's order (`vercmp`) on what precedes a `-`, then a release above
/// its pre-releases (`0.1.1-beta.9` < `0.1.1-beta.10` < `0.1.1`). A pacman version has no `-`
/// once its package release is off: `0.1.2` < `0.1.2.r3.gb8f455d` < `0.1.3`.
pub fn cmp_versions(a: &str, b: &str) -> Ordering {
    let split = |v: &str| match v.split_once('-') {
        Some((base, pre)) => (base.to_string(), Some(pre.to_string())),
        None => (v.to_string(), None),
    };
    let ((ba, pa), (bb, pb)) = (split(a), split(b));
    match vercmp(&ba, &bb) {
        Ordering::Equal => match (pa, pb) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(x), Some(y)) => vercmp(&x, &y),
        },
        o => o,
    }
}

/// pacman's `rpmvercmp`: runs of digits compared as numbers, runs of letters as text, a number
/// above letters; once one side is done, a remaining letter run is older, anything else newer.
fn vercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        while i < a.len() && !a[i].is_ascii_alphanumeric() {
            i += 1;
        }
        while j < b.len() && !b[j].is_ascii_alphanumeric() {
            j += 1;
        }
        if i >= a.len() || j >= b.len() {
            break;
        }
        let num = a[i].is_ascii_digit();
        let kind = |c: u8| {
            if num {
                c.is_ascii_digit()
            } else {
                c.is_ascii_alphabetic()
            }
        };
        let (si, sj) = (i, j);
        while i < a.len() && kind(a[i]) {
            i += 1;
        }
        while j < b.len() && kind(b[j]) {
            j += 1;
        }
        let (x, y) = (&a[si..i], &b[sj..j]);
        if y.is_empty() {
            // Segments of different kinds: the number is newer.
            return if num {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        let o = if num {
            let trim = |s: &[u8]| -> usize { s.iter().take_while(|c| **c == b'0').count() };
            let (x, y) = (&x[trim(x)..], &y[trim(y)..]);
            x.len().cmp(&y.len()).then_with(|| x.cmp(y))
        } else {
            x.cmp(y)
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    let (ra, rb) = (&a[i.min(a.len())..], &b[j.min(b.len())..]);
    if ra.is_empty() && rb.is_empty() {
        Ordering::Equal
    } else if (ra.is_empty() && !rb[0].is_ascii_alphabetic())
        || (!ra.is_empty() && ra[0].is_ascii_alphabetic())
    {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

/// A published release (drafts left out).
#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub tag: String,
    pub prerelease: bool,
}

/// The releases of the app's repository, newest first.
pub fn releases(app: &App, tools: &Tools) -> Result<Vec<Release>> {
    let out = output(
        &tools.gh,
        &[
            "release",
            "list",
            "-R",
            &app.github,
            "--exclude-drafts",
            "--limit",
            "100",
            "--json",
            "tagName,isPrerelease",
            "--jq",
            ".[] | .tagName + \" \" + (.isPrerelease | tostring)",
        ],
    )?;
    Ok(parse_releases(&out))
}

fn parse_releases(out: &str) -> Vec<Release> {
    out.lines()
        .filter_map(|l| l.trim().split_once(' '))
        .map(|(tag, pre)| Release {
            tag: tag.to_string(),
            prerelease: pre == "true",
        })
        .collect()
}

/// `*` matches any text (possibly empty); everything else literally.
pub fn glob(pat: &str, s: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == s;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !s.starts_with(first) || s.len() < first.len() + last.len() || !s.ends_with(last) {
        return false;
    }
    let mut rest = &s[first.len()..s.len() - last.len()];
    for p in &parts[1..parts.len() - 1] {
        match rest.find(p) {
            Some(i) => rest = &rest[i + p.len()..],
            None => return false,
        }
    }
    true
}

/// The version an asset name carries where the template says `{version}` (the shortest that
/// fits), or `None` if the name does not match the template.
pub fn asset_version(template: &str, name: &str) -> Option<String> {
    let Some((before, after)) = template.split_once("{version}") else {
        return glob(template, name).then(String::new);
    };
    // A version starts with a digit (after an optional `v`): `vocaline-debug-0.1.2-1-…`, the
    // debug package of the same release, is not version `debug`.
    let starts_well = |v: &str| {
        v.strip_prefix('v')
            .unwrap_or(v)
            .starts_with(|c: char| c.is_ascii_digit())
    };
    let cuts: Vec<usize> = (0..=name.len())
        .filter(|&k| name.is_char_boundary(k))
        .collect();
    for &s in &cuts {
        if !glob(before, &name[..s]) {
            continue;
        }
        for &e in cuts.iter().filter(|&&e| e > s) {
            let v = &name[s..e];
            if starts_well(v) && glob(after, &name[e..]) {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// An asset to download: its name, the sha256 GitHub publishes, the version its name carries
/// (the tag's when the template has no `{version}`).
#[derive(Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub sum: String,
    pub version: String,
}

/// The asset of release `tag` matching the template, from `name digest` lines.
pub fn pick_asset(app: &App, tag: &str, listing: &str) -> Result<Asset> {
    let found: Vec<(&str, &str, String)> = listing
        .lines()
        .filter_map(|l| l.trim().split_once(' '))
        .filter_map(|(name, d)| asset_version(&app.asset, name).map(|v| (name, d, v)))
        .collect();
    match found.as_slice() {
        [(name, digest, v)] => {
            let Some(sum) = digest.strip_prefix("sha256:") else {
                bail!("{tag}: no sha256 published by GitHub for {name}, nothing installed");
            };
            let version = if v.is_empty() { norm(tag) } else { norm(v) };
            Ok(Asset {
                name: name.to_string(),
                sum: sum.to_string(),
                version,
            })
        }
        [] => bail!("{tag}: no asset {} in the release", app.asset),
        _ => bail!("{tag}: several assets match {}", app.asset),
    }
}

fn asset_of(app: &App, tag: &str, tools: &Tools) -> Result<Asset> {
    let listing = output(
        &tools.gh,
        &[
            "release",
            "view",
            tag,
            "-R",
            &app.github,
            "--json",
            "assets",
            "--jq",
            ".assets[] | .name + \" \" + (.digest // \"none\")",
        ],
    )?;
    pick_asset(app, tag, &listing)
}

/// What to install: the release and the version its asset carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub tag: String,
    pub version: String,
}

/// The latest release, and with `prerelease` the latest pre-release too: the higher version of
/// the two (a rolling pre-release such as `dev-latest` carries its version in its asset).
pub fn target(app: &App, rel: &[Release], tools: &Tools) -> Result<Target> {
    let stable = rel.iter().find(|r| !r.prerelease);
    let pre = rel.iter().find(|r| r.prerelease).filter(|_| app.prerelease);
    let mut best: Option<Target> = None;
    let mut errors = Vec::new();
    for r in [stable, pre].into_iter().flatten() {
        // One of the two without a matching asset does not hide the other.
        let a = match asset_of(app, &r.tag, tools) {
            Ok(a) => a,
            Err(e) => {
                errors.push(format!("{e:#}"));
                continue;
            }
        };
        let t = Target {
            tag: r.tag.clone(),
            version: a.version,
        };
        if best
            .as_ref()
            .is_none_or(|b| cmp_versions(&t.version, &b.version) == Ordering::Greater)
        {
            best = Some(t);
        }
    }
    match best {
        Some(t) => Ok(t),
        None if errors.is_empty() => bail!("{}: no release published", app.github),
        None => bail!("{}", errors.join("; ")),
    }
}

/// Where the installed version stands against the target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Standing {
    Missing,
    Behind,
    UpToDate,
    /// Newer than the target (a pre-release, a local build): never moved back.
    Ahead,
    /// Not a published release, and no safe order with the target (`1.0.0` against
    /// `1.0.0-local.<hash>`): shown, left alone.
    Unknown,
}

/// Compares by version; when both are the versions of published tags, by release order (a
/// `1.0.0-local.<hash>` scheme has no version order).
pub fn standing(installed: Option<&str>, t: &Target, rel: &[Release]) -> Standing {
    let Some(i) = installed else {
        return Standing::Missing;
    };
    if i == t.version {
        return Standing::UpToDate;
    }
    let pos = |v: &str| rel.iter().position(|r| norm(&r.tag) == v);
    let (bi, si) = split_pre(i);
    let (bt, st) = split_pre(&t.version);
    // Same base, and a suffix that carries a commit hash on one side: `1.0.0-local.<hash>`
    // has no version order, only the order of its releases; without both in the list, unknown.
    if bi == bt && (has_hash(si) || has_hash(st)) {
        let pt = pos(&norm(&t.tag)).filter(|_| norm(&t.tag) == t.version);
        return match (pos(i), pt) {
            // Newest first: the installed one is older when it comes after the target.
            (Some(pi), Some(pt)) => from_order(pt.cmp(&pi)),
            _ => Standing::Unknown,
        };
    }
    from_order(cmp_versions(i, &t.version))
}

fn from_order(o: Ordering) -> Standing {
    match o {
        Ordering::Less => Standing::Behind,
        Ordering::Equal => Standing::UpToDate,
        Ordering::Greater => Standing::Ahead,
    }
}

/// `1.0.0-local.abc` -> (`1.0.0`, `local.abc`); no `-`: an empty suffix.
fn split_pre(v: &str) -> (&str, &str) {
    v.split_once('-').unwrap_or((v, ""))
}

/// A segment that looks like a commit hash: 7 hex characters or more, digits and letters.
fn has_hash(s: &str) -> bool {
    s.split(|c: char| !c.is_ascii_alphanumeric()).any(|w| {
        w.len() >= 7
            && w.chars().all(|c| c.is_ascii_hexdigit())
            && w.chars().any(|c| c.is_ascii_digit())
            && w.chars().any(|c| c.is_ascii_alphabetic())
    })
}

fn sha256_file(p: &Path) -> Result<String> {
    let mut f = std::fs::File::open(p).with_context(|| format!("reading {}", p.display()))?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Downloads the asset of `tag` into `dir` and checks its sha256; returns its path.
pub fn download(app: &App, tag: &str, dir: &Path, tools: &Tools) -> Result<PathBuf> {
    let a = asset_of(app, tag, tools)?;
    std::fs::create_dir_all(dir)?;
    let file = dir.join(&a.name);
    let _ = std::fs::remove_file(&file);
    let d = dir.to_string_lossy();
    output(
        &tools.gh,
        &[
            "release",
            "download",
            tag,
            "-R",
            &app.github,
            "--pattern",
            &a.name,
            "-D",
            &d,
            "--clobber",
        ],
    )?;
    let got = sha256_file(&file)?;
    if got != a.sum {
        let _ = std::fs::remove_file(&file);
        bail!(
            "{}: wrong sha256 ({got}, expected {}), removed, nothing installed",
            a.name,
            a.sum
        );
    }
    Ok(file)
}

/// What an installation did, and what it left to run with sudo (a pacman package).
pub struct Installed {
    pub target: Target,
    pub by_hand: Option<Vec<String>>,
}

/// Installs `target` (the one `target()` picks if `None`). `busy` and paths already
/// substituted. `cache` keeps the packages to install with sudo; the other archives go through
/// a temporary folder in it, removed after.
pub fn install(
    app: &App,
    target: Option<&Target>,
    busy: &[String],
    bin_dir: &Path,
    cache: &Path,
    tools: &Tools,
) -> Result<Installed> {
    let how = app.install()?;
    if !busy.is_empty() && succeeds(busy) {
        bail!(
            "{}: in use, not replaced now (run the command again later)",
            app.id
        );
    }
    let t = match target {
        Some(t) => t.clone(),
        None => self::target(app, &releases(app, tools)?, tools)?,
    };
    if let Install::Package(_) = how {
        let file = download(app, &t.tag, &cache.join("apps"), tools)?;
        return Ok(Installed {
            by_hand: Some(vec![
                "sudo".into(),
                "pacman".into(),
                "-U".into(),
                file.to_string_lossy().into_owned(),
            ]),
            target: t,
        });
    }
    let tmp = cache.join(format!("apps/.{}-{}", app.id, std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let res = (|| {
        let archive = download(app, &t.tag, &tmp, tools)?;
        let x = tmp.join("x");
        std::fs::create_dir_all(&x)?;
        let st = Command::new("tar")
            .arg("-xf")
            .arg(&archive)
            .arg("-C")
            .arg(&x)
            .status()
            .context("cannot run tar")?;
        if !st.success() {
            bail!("{}: extraction failed", archive.display());
        }
        match how {
            Install::Bin(b) => {
                let b = b.replace("{version}", &t.version);
                let src = x.join(&b);
                if !src.is_file() {
                    bail!("{b}: not in the archive");
                }
                let name = Path::new(&b).file_name().context("empty bin")?;
                std::fs::create_dir_all(bin_dir)?;
                let new = bin_dir.join(format!(".{}.new", name.to_string_lossy()));
                std::fs::copy(&src, &new)?;
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))?;
                std::fs::rename(&new, bin_dir.join(name))?;
            }
            Install::Script(s) => {
                let script = x.join(s.replace("{version}", &t.version));
                if !script.is_file() {
                    bail!("{}: not in the archive", script.display());
                }
                let st = Command::new(&script)
                    .current_dir(script.parent().unwrap_or(&x))
                    .status()
                    .with_context(|| format!("cannot run {}", script.display()))?;
                if !st.success() {
                    bail!("{} failed ({st})", script.display());
                }
            }
            Install::Package(_) => unreachable!(),
        }
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    res.map(|()| Installed {
        target: t,
        by_hand: None,
    })
}

// ------------------------------------------------------------------ plugins

/// Target of a plugin without a pinned commit: its most recent version tag if it has tags,
/// otherwise the head of its default branch. From `git ls-remote --tags --sort=-v:refname
/// origin` then `git ls-remote origin HEAD`: (name, commit).
pub fn plugin_target(tags: &str, head: &str, prerelease: bool) -> Option<(String, String)> {
    // A release first (no `-rc1`, `-beta` suffix), unless pre-releases are followed; a
    // pre-release also when there is nothing else.
    first_tag(tags, prerelease)
        .or_else(|| first_tag(tags, true))
        .or_else(|| {
            let sha = head.split_whitespace().next()?;
            Some(("default branch".into(), sha.to_string()))
        })
}

/// The first tag of the (sorted) `ls-remote` listing, with its commit (the peeled line of an
/// annotated tag). `pre`: pre-release tags (with a `-`) count too.
fn first_tag(tags: &str, pre: bool) -> Option<(String, String)> {
    let mut first: Option<(String, String)> = None;
    for l in tags.lines() {
        let Some((sha, r)) = l.split_once('\t') else {
            continue;
        };
        let Some(name) = r.strip_prefix("refs/tags/") else {
            continue;
        };
        let (name, peeled) = match name.strip_suffix("^{}") {
            Some(n) => (n, true),
            None => (name, false),
        };
        if !pre && name.contains('-') {
            continue;
        }
        match &first {
            None => first = Some((name.to_string(), sha.to_string())),
            // An annotated tag: the commit is on the peeled line.
            Some((n, _)) if n == name && peeled => {
                first = Some((name.to_string(), sha.to_string()))
            }
            Some(_) => {}
        }
    }
    first
}

/// Where the plugin is and where it can go (`git ls-remote`, nothing written).
pub struct PluginState {
    pub head: String,
    pub target: Option<(String, String)>,
    /// The target is already in `HEAD` (a plugin moved past its latest tag): nothing to do,
    /// never a step back.
    pub ahead: bool,
}

pub fn plugin_state(dir: &Path, prerelease: bool, tools: &Tools) -> Result<PluginState> {
    let d = dir.to_string_lossy();
    let head = output(&tools.git, &["-C", &d, "rev-parse", "HEAD"])?;
    let tags = output(
        &tools.git,
        &[
            "-C",
            &d,
            "-c",
            "versionsort.suffix=-",
            "ls-remote",
            "--tags",
            "--sort=-v:refname",
            "origin",
        ],
    )?;
    // The default branch only matters without a tag.
    let remote_head = if tags.is_empty() {
        output(&tools.git, &["-C", &d, "ls-remote", "origin", "HEAD"])?
    } else {
        String::new()
    };
    let target = plugin_target(&tags, &remote_head, prerelease);
    let ahead = target.as_ref().is_some_and(|(_, c)| {
        *c != head
            && output(
                &tools.git,
                &["-C", &d, "merge-base", "--is-ancestor", c, "HEAD"],
            )
            .is_ok()
    });
    Ok(PluginState {
        head,
        target,
        ahead,
    })
}

/// Name of what a plugin is at: its tag if `HEAD` has one, otherwise the short commit.
pub fn describe(dir: &Path, tools: &Tools) -> String {
    let d = dir.to_string_lossy();
    output(
        &tools.git,
        &["-C", &d, "describe", "--tags", "--exact-match", "HEAD"],
    )
    .or_else(|_| output(&tools.git, &["-C", &d, "rev-parse", "--short", "HEAD"]))
    .unwrap_or_else(|_| "?".into())
}

fn git(tools: &Tools, dir: &Path, args: &[&str]) -> Result<String> {
    let d = dir.to_string_lossy();
    let mut all = vec!["-C", &*d, "-c", "advice.detachedHead=false"];
    all.extend_from_slice(args);
    output(&tools.git, &all)
}

/// Commits between the plugin's `HEAD` and `commit`, for the review. `fetch`: fetches them
/// first (otherwise only what the clone already has: a dry run writes nothing).
pub fn plugin_log(dir: &Path, commit: &str, fetch: bool, tools: &Tools) -> Result<String> {
    if fetch {
        git(tools, dir, &["fetch", "--quiet", "--tags", "origin"])?;
    }
    git(
        tools,
        dir,
        &[
            "log",
            "--oneline",
            "--no-decorate",
            &format!("HEAD..{commit}"),
        ],
    )
}

/// A plugin's local patch: the repository's file (`None`: removed from the steps), and the copy
/// of what poste last applied (`~/.local/state/poste/patches/<id>.patch`). The copy is what lets
/// the patch be taken off once the repository's file has changed or gone.
#[derive(Clone, Debug)]
pub struct Patch {
    pub file: Option<PathBuf>,
    pub applied: PathBuf,
}

impl Patch {
    fn reverses(p: &Path, dir: &Path, tools: &Tools) -> bool {
        p.is_file()
            && git(
                tools,
                dir,
                &["apply", "--reverse", "--check", &p.to_string_lossy()],
            )
            .is_ok()
    }

    /// Changed lines of a patch.
    fn weight(p: &Path) -> usize {
        std::fs::read_to_string(p)
            .unwrap_or_default()
            .lines()
            .filter(|l| {
                (l.starts_with('+') && !l.starts_with("+++"))
                    || (l.starts_with('-') && !l.starts_with("---"))
            })
            .count()
    }

    /// The patch that is on the plugin, among the copy (what poste applied) and the repository's
    /// file; `None` if neither can be taken off. When both can, one holds the other (a patch
    /// grown or cut down): the larger is the one on it, the other would leave changes behind.
    pub fn on(&self, dir: &Path, tools: &Tools) -> Option<PathBuf> {
        std::iter::once(&self.applied)
            .chain(self.file.as_ref())
            .filter(|p| Self::reverses(p, dir, tools))
            .fold(None, |best: Option<&PathBuf>, p| match best {
                Some(b) if Self::weight(b) >= Self::weight(p) => Some(b),
                _ => Some(p),
            })
            .cloned()
    }

    /// The copy is the repository's file (or both are absent).
    pub fn recorded(&self) -> bool {
        match &self.file {
            Some(f) => match (std::fs::read(f), std::fs::read(&self.applied)) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            },
            None => !self.applied.exists(),
        }
    }

    /// The plugin carries an earlier version of the patch (or one removed from the steps).
    pub fn stale(&self, on: Option<&PathBuf>) -> bool {
        on == Some(&self.applied) && !self.recorded()
    }

    /// `record` once the patch is on the plugin: failing to keep the copy is only a warning (the
    /// copy left over no longer matches the plugin, the repository's file is found on it).
    pub fn record_or_warn(&self) {
        if let Err(e) = self.record() {
            eprintln!("    warning: {e:#}");
        }
    }

    /// Keeps a copy of the repository's file as the applied patch (removes it if there is none).
    pub fn record(&self) -> Result<()> {
        let Some(file) = &self.file else {
            if self.applied.exists() {
                std::fs::remove_file(&self.applied)?;
            }
            return Ok(());
        };
        if let Some(parent) = self.applied.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(file, &self.applied)
            .with_context(|| format!("copying {} to {}", file.display(), self.applied.display()))?;
        Ok(())
    }
}

/// Moves a plugin to `commit`, its local patch taken off (the version that is on it, see
/// [`Patch::on`]) then the repository's put on and recorded. If that no longer applies, the
/// plugin is put back where it was, patch included, and the update fails. `commit` may be
/// `HEAD`: then nothing is fetched nor checked out, only the patch is renewed.
pub fn plugin_move(dir: &Path, commit: &str, patch: Option<&Patch>, tools: &Tools) -> Result<()> {
    let old = git(tools, dir, &["rev-parse", "HEAD"])?;
    let moving = commit != old;
    // The network first: once the patch is off, nothing should be left that can fail alone.
    if moving {
        git(tools, dir, &["fetch", "--quiet", "--tags", "origin"])?;
    }
    let status = |tools: &Tools| {
        git(
            tools,
            dir,
            &["status", "--porcelain", "--untracked-files=no"],
        )
    };
    // The patch taken off, to put back if anything fails.
    let mut off: Option<String> = None;
    if !status(tools)?.is_empty() {
        match patch.and_then(|p| p.on(dir, tools)) {
            Some(p) => {
                let p = p.to_string_lossy().into_owned();
                git(tools, dir, &["apply", "--reverse", &p])?;
                // A file the patch created and that was marked `git add -N`: its empty index
                // entry stays once the file is gone, and is only a placeholder.
                let summary = git(tools, dir, &["apply", "--summary", &p])?;
                for f in summary
                    .lines()
                    .filter_map(|l| l.trim().strip_prefix("create mode "))
                    .filter_map(|l| l.split_once(' ').map(|(_, f)| f))
                {
                    let in_head =
                        git(tools, dir, &["cat-file", "-e", &format!("HEAD:{f}")]).is_ok();
                    if !in_head && !dir.join(f).exists() {
                        git(
                            tools,
                            dir,
                            &["rm", "--cached", "--quiet", "--ignore-unmatch", "--", f],
                        )?;
                    }
                }
                if !status(tools)?.is_empty() {
                    git(tools, dir, &["apply", &p])?;
                    bail!(
                        "{}: files modified by hand besides the patch, nothing touched",
                        dir.display()
                    );
                }
                off = Some(p);
            }
            None => bail!("{}: files modified by hand, nothing touched", dir.display()),
        }
    }
    // From here on, any failure puts the plugin back where it was, patch included.
    let back = |why: anyhow::Error| -> Result<()> {
        let checkout = if moving {
            git(tools, dir, &["checkout", "--quiet", "--detach", &old]).map(|_| ())
        } else {
            Ok(())
        };
        checkout
            .and_then(|_| match &off {
                Some(p) => git(tools, dir, &["apply", p]).map(|_| ()),
                None => Ok(()),
            })
            .with_context(|| {
                format!(
                    "{why:#}; and putting {} back at {} failed: to repair by hand",
                    dir.display(),
                    &old[..old.len().min(7)]
                )
            })?;
        Err(why)
    };
    if moving {
        if let Err(e) = git(tools, dir, &["checkout", "--quiet", "--detach", commit]) {
            return back(e);
        }
    }
    if let Some(patch) = patch {
        if let Some(file) = &patch.file {
            let p = file.to_string_lossy().into_owned();
            if git(tools, dir, &["apply", "--check", &p]).is_err() {
                return back(anyhow::anyhow!(
                    "the patch {p} no longer applies to {}: plugin left where it was, update the patch",
                    &commit[..commit.len().min(7)]
                ));
            }
            if let Err(e) = git(tools, dir, &["apply", &p]) {
                return back(e);
            }
        }
        patch.record_or_warn();
    }
    Ok(())
}

/// Where a plugin goes: its target, or where it is when the target is already in `HEAD` (never
/// moved back).
pub fn destination<'a>(head: &'a str, target: &'a str, ahead: bool) -> &'a str {
    if ahead {
        head
    } else {
        target
    }
}

/// No tracked file of the plugin is changed.
pub fn plugin_clean(dir: &Path, tools: &Tools) -> bool {
    git(
        tools,
        dir,
        &["status", "--porcelain", "--untracked-files=no"],
    )
    .is_ok_and(|s| s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn app(toml_text: &str) -> App {
        toml::from_str(toml_text).unwrap()
    }

    #[test]
    fn versions_are_normalized() {
        assert_eq!(norm("v0.1.2"), "0.1.2");
        assert_eq!(norm("melodie 0.1.1-beta.9"), "0.1.1-beta.9");
        assert_eq!(norm("strata v1.0.0-local.8561a63"), "1.0.0-local.8561a63");
        assert_eq!(strip_pkgrel("0.1.2-1"), "0.1.2");
        assert_eq!(strip_pkgrel("1.0.0-local.abc"), "1.0.0-local.abc");
        assert_eq!(strip_pkgrel("4.0.4-12"), "4.0.4");
    }

    #[test]
    fn glob_matches() {
        assert!(glob("a-*-x.zst", "a-1-x.zst"));
        assert!(glob("a-*-x.zst", "a--x.zst"));
        assert!(!glob("a-*-x.zst", "a-1-y.zst"));
        assert!(glob("exact", "exact"));
        assert!(!glob("exact", "exact2"));
        assert!(glob("*", "anything"));
        assert!(glob("a*b*c", "a12b34c"));
        assert!(!glob("a*b*c", "ac"));
        assert!(!glob("ab*ba", "aba"));
    }

    #[test]
    fn install_kind_is_exactly_one() {
        let base = "id=\"a\"\ntitle=\"t\"\ngithub=\"o/a\"\nasset=\"a\"\n";
        assert_eq!(
            app(&format!("{base}bin=\"a\"\nversion=[\"a\"]"))
                .install()
                .unwrap(),
            Install::Bin("a")
        );
        assert!(
            app(&format!("{base}bin=\"a\"")).install().is_err(),
            "without version it would be reinstalled at every setup"
        );
        assert_eq!(
            app(&format!("{base}package=\"a\"")).install().unwrap(),
            Install::Package("a")
        );
        assert!(toml::from_str::<App>(&format!("{base}bin=\"a\"\ncommit=\"x\"")).is_err());
        assert!(app(base).install().is_err());
        assert!(app(&format!("{base}bin=\"a\"\npackage=\"a\""))
            .install()
            .is_err());
    }

    #[test]
    fn asset_is_picked_with_its_checksum() {
        let a = app(
            "id=\"v\"\ntitle=\"t\"\ngithub=\"o/v\"\nasset=\"v-{version}-*-x86_64.pkg.tar.zst\"\npackage=\"v\"",
        );
        let listing = "v-0.2.0-1-x86_64.pkg.tar.zst sha256:abc\nnotes.txt sha256:def\n";
        assert_eq!(
            pick_asset(&a, "v0.2.0", listing).unwrap(),
            Asset {
                name: "v-0.2.0-1-x86_64.pkg.tar.zst".into(),
                sum: "abc".into(),
                version: "0.2.0".into()
            }
        );
        // A rolling pre-release: the version is in the asset, not in the tag.
        let dev = "v-0.1.2.r3.gb8f455d-1-x86_64.pkg.tar.zst sha256:abc\n";
        assert_eq!(
            pick_asset(&a, "dev-latest", dev).unwrap().version,
            "0.1.2.r3.gb8f455d"
        );
        assert!(
            pick_asset(&a, "v0.3.0", "notes.txt sha256:d\n").is_err(),
            "absent"
        );
        assert!(
            pick_asset(&a, "v0.2.0", "v-0.2.0-1-x86_64.pkg.tar.zst none\n").is_err(),
            "no checksum, no install"
        );
        let two = "v-0.2.0-1-x86_64.pkg.tar.zst sha256:a\nv-0.2.0-2-x86_64.pkg.tar.zst sha256:b\n";
        assert!(pick_asset(&a, "v0.2.0", two).is_err(), "ambiguous");
    }

    #[test]
    fn plugin_target_prefers_the_latest_tag() {
        let tags = "aaa\trefs/tags/v1.2.0\nbbb\trefs/tags/v1.2.0^{}\nccc\trefs/tags/v1.1.0\n";
        assert_eq!(
            plugin_target(tags, "fff\tHEAD", false),
            Some(("v1.2.0".into(), "bbb".into()))
        );
        let light = "ddd\trefs/tags/v2.0.0\nccc\trefs/tags/v1.1.0\n";
        assert_eq!(
            plugin_target(light, "fff\tHEAD", false),
            Some(("v2.0.0".into(), "ddd".into()))
        );
        assert_eq!(
            plugin_target("", "fff\tHEAD", false),
            Some(("default branch".into(), "fff".into()))
        );
        assert_eq!(plugin_target("", "", false), None);
        // A release before a pre-release; a pre-release only when there is nothing else.
        let rc = "eee\trefs/tags/v3.0.0-rc1\nddd\trefs/tags/v2.0.0\n";
        assert_eq!(
            plugin_target(rc, "", false),
            Some(("v2.0.0".into(), "ddd".into()))
        );
        assert_eq!(
            plugin_target(rc, "", true),
            Some(("v3.0.0-rc1".into(), "eee".into())),
            "pre-releases followed"
        );
        assert_eq!(
            plugin_target("eee\trefs/tags/v3.0.0-rc1\n", "", false),
            Some(("v3.0.0-rc1".into(), "eee".into()))
        );
    }

    /// A fake `gh`: serves `tag`, its asset (a tar with `strata` inside) and a digest.
    fn fake_gh(dir: &Path, tag: &str, asset: &Path, digest: &str) -> Vec<String> {
        let script = dir.join("gh");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\n\
                 case \"$*\" in\n\
                 *'--json tagName,isPrerelease'*) echo '{tag} false' ;;\n\
                 *'--json assets'*) echo \"$(basename '{a}') {digest}\" ;;\n\
                 *download*) while [ \"$1\" != -D ]; do shift; done; cp '{a}' \"$2/\" ;;\n\
                 *) exit 1 ;;\n\
                 esac\n",
                a = asset.display()
            ),
        )
        .unwrap();
        // Through sh, never executed itself: a script just written can be "busy" for exec
        // while another test thread forks.
        vec!["sh".into(), script.to_string_lossy().into_owned()]
    }

    fn archive(dir: &Path) -> PathBuf {
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("strata"), "#!/bin/sh\necho strata v1.2.0\n").unwrap();
        let a = dir.join("strata-x86_64.tar.gz");
        let st = Command::new("tar")
            .arg("-czf")
            .arg(&a)
            .arg("-C")
            .arg(&src)
            .arg("strata")
            .status()
            .unwrap();
        assert!(st.success());
        a
    }

    #[test]
    fn bin_installed_after_checksum_and_read_back() {
        let d = tempfile::tempdir().unwrap();
        let a = archive(d.path());
        let sum = sha256_file(&a).unwrap();
        let tools = Tools {
            gh: fake_gh(d.path(), "v1.2.0", &a, &format!("sha256:{sum}")),
            ..Default::default()
        };
        let bin = d.path().join("bin");
        let app = app(&format!(
            "id=\"strata\"\ntitle=\"t\"\ngithub=\"o/strata\"\nasset=\"strata-x86_64.tar.gz\"\nbin=\"strata\"\nversion=[\"sh\", \"{}\", \"--version\"]",
            bin.join("strata").display()
        ));
        assert_eq!(installed(&app, &app.version, &tools), None);
        let done = install(&app, None, &[], &bin, &d.path().join("cache"), &tools).unwrap();
        assert_eq!(done.target.tag, "v1.2.0");
        assert_eq!(done.target.version, "1.2.0");
        assert!(done.by_hand.is_none());
        assert_eq!(
            installed(&app, &app.version, &tools).as_deref(),
            Some("1.2.0")
        );
        assert!(
            !d.path()
                .join("cache/apps")
                .read_dir()
                .unwrap()
                .any(|_| true),
            "temporary folder removed"
        );
        // Busy: nothing replaced.
        let busy = vec!["true".to_string()];
        assert!(install(&app, None, &busy, &bin, &d.path().join("cache"), &tools).is_err());
    }

    #[test]
    fn bin_in_a_versioned_folder() {
        // Upstream Strata: strata-<version>-<target>.tar.gz holds strata-<version>-<target>/strata.
        let d = tempfile::tempdir().unwrap();
        let top = "strata-1.2.0-x86_64-unknown-linux-gnu";
        let src = d.path().join("src").join(top);
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("strata"), "#!/bin/sh\necho strata 1.2.0\n").unwrap();
        let a = d.path().join(format!("{top}.tar.gz"));
        let st = Command::new("tar")
            .arg("-czf")
            .arg(&a)
            .arg("-C")
            .arg(d.path().join("src"))
            .arg(top)
            .status()
            .unwrap();
        assert!(st.success());
        let sum = sha256_file(&a).unwrap();
        let tools = Tools {
            gh: fake_gh(d.path(), "v1.2.0", &a, &format!("sha256:{sum}")),
            ..Default::default()
        };
        let bin = d.path().join("bin");
        let app = app(&format!(
            "id=\"strata\"\ntitle=\"t\"\ngithub=\"o/strata\"\nasset=\"strata-{{version}}-x86_64-unknown-linux-gnu.tar.gz\"\nbin=\"strata-{{version}}-x86_64-unknown-linux-gnu/strata\"\nversion=[\"sh\", \"{}\", \"--version\"]",
            bin.join("strata").display()
        ));
        install(&app, None, &[], &bin, &d.path().join("cache"), &tools).unwrap();
        assert_eq!(
            installed(&app, &app.version, &tools).as_deref(),
            Some("1.2.0")
        );
    }

    #[test]
    fn wrong_checksum_installs_nothing() {
        let d = tempfile::tempdir().unwrap();
        let a = archive(d.path());
        let tools = Tools {
            gh: fake_gh(d.path(), "v1.2.0", &a, "sha256:0000"),
            ..Default::default()
        };
        let bin = d.path().join("bin");
        let app = app(
            "id=\"strata\"\ntitle=\"t\"\ngithub=\"o/strata\"\nasset=\"strata-x86_64.tar.gz\"\nbin=\"strata\"\nversion=[\"strata\"]",
        );
        assert!(install(&app, None, &[], &bin, &d.path().join("cache"), &tools).is_err());
        assert!(!bin.join("strata").exists());
    }

    #[test]
    fn package_is_downloaded_then_left_to_pacman() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("v-0.2.0-1-x86_64.pkg.tar.zst");
        fs::write(&a, "pkg").unwrap();
        let sum = sha256_file(&a).unwrap();
        let pacman = d.path().join("pacman");
        fs::write(&pacman, "echo \"v 0.1.2-1\"\n").unwrap();
        let tools = Tools {
            gh: fake_gh(d.path(), "v0.2.0", &a, &format!("sha256:{sum}")),
            pacman: vec!["sh".into(), pacman.to_string_lossy().into_owned()],
            ..Default::default()
        };
        let app = app(
            "id=\"v\"\ntitle=\"t\"\ngithub=\"o/v\"\nasset=\"v-{version}-*-x86_64.pkg.tar.zst\"\npackage=\"v\"",
        );
        assert_eq!(installed(&app, &[], &tools).as_deref(), Some("0.1.2"));
        let cache = d.path().join("cache");
        let done = install(&app, None, &[], &d.path().join("bin"), &cache, &tools).unwrap();
        let kept = cache.join("apps/v-0.2.0-1-x86_64.pkg.tar.zst");
        assert!(kept.is_file());
        assert_eq!(
            done.by_hand.unwrap().join(" "),
            format!("sudo pacman -U {}", kept.display())
        );
    }

    #[test]
    fn versions_are_ordered_like_pacman_then_semver() {
        use Ordering::*;
        let c = cmp_versions;
        assert_eq!(c("0.1.2", "0.1.2.r3.gb8f455d"), Less);
        assert_eq!(c("0.1.2.r3.gb8f455d", "0.1.2.r10.g0000000"), Less);
        assert_eq!(c("0.1.2.r10.g0000000", "0.1.3"), Less);
        assert_eq!(c("0.1.1-beta.9", "0.1.1-beta.10"), Less);
        assert_eq!(c("0.1.1-beta.10", "0.1.1"), Less);
        assert_eq!(c("0.1.0", "0.1.1-beta.9"), Less);
        assert_eq!(c("1.10.0", "1.9.0"), Greater);
        assert_eq!(c("1.0a", "1.0"), Less);
        assert_eq!(c("2.0", "2.0"), Equal);
    }

    #[test]
    fn asset_version_is_read_from_the_name() {
        let t = "v-{version}-*-x86_64.pkg.tar.zst";
        assert_eq!(
            asset_version(t, "v-0.1.2.r3.gb8f455d-1-x86_64.pkg.tar.zst").as_deref(),
            Some("0.1.2.r3.gb8f455d")
        );
        assert_eq!(asset_version(t, "other-1-1-x86_64.pkg.tar.zst"), None);
        assert_eq!(
            asset_version("m-{version}-linux.tar.gz", "m-0.1.1-beta.10-linux.tar.gz").as_deref(),
            Some("0.1.1-beta.10")
        );
        assert_eq!(asset_version("s.tar.gz", "s.tar.gz").as_deref(), Some(""));
        // The debug package of the same release is not version `debug`.
        assert_eq!(asset_version(t, "v-debug-0.1.2-1-x86_64.pkg.tar.zst"), None);
        assert_eq!(
            asset_version("*-{version}-linux.tar.gz", "tool-2.1.0-linux.tar.gz").as_deref(),
            Some("2.1.0")
        );
    }

    #[test]
    fn never_moved_back() {
        let rel = |l: &str| parse_releases(l);
        let t = |tag: &str, v: &str| Target {
            tag: tag.into(),
            version: v.into(),
        };
        let none: Vec<Release> = vec![];
        assert_eq!(standing(None, &t("v1", "1"), &none), Standing::Missing);
        // A beta installed, the channel stable: the older stable is not an update.
        let m = rel("v0.1.1-beta.10 true\nv0.1.0 false\n");
        assert_eq!(
            standing(Some("0.1.1-beta.9"), &t("v0.1.0", "0.1.0"), &m),
            Standing::Ahead
        );
        assert_eq!(
            standing(
                Some("0.1.1-beta.9"),
                &t("v0.1.1-beta.10", "0.1.1-beta.10"),
                &m
            ),
            Standing::Behind
        );
        // A dev build installed, the stable it starts from is older.
        let v = rel("dev-latest true\nv0.1.2 false\n");
        assert_eq!(
            standing(Some("0.1.2.r3.gb8f455d"), &t("v0.1.2", "0.1.2"), &v),
            Standing::Ahead
        );
        assert_eq!(
            standing(
                Some("0.1.2.r3.gb8f455d"),
                &t("dev-latest", "0.1.2.r3.gb8f455d"),
                &v
            ),
            Standing::UpToDate
        );
        // A version that is no release, against a suffixed one of the same base: unknown.
        assert_eq!(
            standing(
                Some("1.0.0"),
                &t("v1.0.0-local.8b31b95", "1.0.0-local.8b31b95"),
                &rel("v1.0.0-local.8b31b95 false\n")
            ),
            Standing::Unknown
        );
        // A hotfix of an older line published after a newer release: no step back.
        assert_eq!(
            standing(
                Some("2.0.0"),
                &t("v1.2.5", "1.2.5"),
                &rel("v1.2.5 false\nv2.0.0 false\n")
            ),
            Standing::Ahead
        );
        // Hashes and no release to place the installed one: unknown, left alone.
        assert_eq!(
            standing(
                Some("1.0.0-local.8561a63"),
                &t("v1.0.0-local.3b24e32", "1.0.0-local.3b24e32"),
                &none
            ),
            Standing::Unknown
        );
        // Versions without an order (a hash): the release order decides.
        let s = rel("v1.0.0-local.3b24e32 false\nv1.0.0-local.8561a63 false\n");
        assert_eq!(
            standing(
                Some("1.0.0-local.8561a63"),
                &t("v1.0.0-local.3b24e32", "1.0.0-local.3b24e32"),
                &s
            ),
            Standing::Behind
        );
    }

    fn sh(dir: &Path, args: &[&str]) {
        let st = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "tag.gpgSign=false",
            ])
            .args(args)
            .status()
            .unwrap();
        assert!(st.success(), "git {args:?}");
    }

    #[test]
    fn plugin_moves_with_its_patch_or_stays() {
        let d = tempfile::tempdir().unwrap();
        let up = d.path().join("up");
        fs::create_dir_all(&up).unwrap();
        sh(&up, &["init", "-q"]);
        fs::write(up.join("f.txt"), "one\nkeep\n").unwrap();
        sh(&up, &["add", "."]);
        sh(&up, &["commit", "-q", "-m", "1"]);
        let plug = d.path().join("plug");
        let st = Command::new("git")
            .args(["clone", "-q"])
            .arg(&up)
            .arg(&plug)
            .status()
            .unwrap();
        assert!(st.success());
        let file = d.path().join("p.patch");
        fs::write(
            &file,
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-one\n+two\n keep\n\
             diff --git a/n.txt b/n.txt\nnew file mode 100644\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1 @@\n+new\n",
        )
        .unwrap();
        sh(&plug, &["apply", &file.to_string_lossy()]);
        let patch = Patch {
            file: Some(file.clone()),
            applied: d.path().join("state/patches/p.patch"),
        };
        // The created file marked intent-to-add, as found on a real plugin.
        sh(&plug, &["add", "-N", "n.txt"]);
        let tools = Tools::default();

        // Upstream changes another line: the patch follows.
        fs::write(up.join("g.txt"), "new\n").unwrap();
        sh(&up, &["add", "."]);
        sh(&up, &["commit", "-q", "-m", "2"]);
        sh(&up, &["tag", "v2"]);
        let st = plugin_state(&plug, false, &tools).unwrap();
        let (name, c2) = st.target.unwrap();
        assert_eq!(name, "v2");
        assert_ne!(st.head, c2);
        assert!(plugin_log(&plug, &c2, true, &tools).unwrap().contains(" 2"));
        plugin_move(&plug, &c2, Some(&patch), &tools).unwrap();
        assert_eq!(
            fs::read_to_string(plug.join("f.txt")).unwrap(),
            "two\nkeep\n"
        );
        assert!(plug.join("g.txt").is_file());
        assert_eq!(fs::read_to_string(plug.join("n.txt")).unwrap(), "new\n");
        assert_eq!(describe(&plug, &tools), "v2");
        assert!(patch.recorded());

        // Upstream changes the patched line: left at v2, patch still there.
        fs::write(up.join("f.txt"), "uno\nkeep\n").unwrap();
        sh(&up, &["commit", "-q", "-am", "3"]);
        let c3 = git(&tools, &up, &["rev-parse", "HEAD"]).unwrap();
        assert!(plugin_move(&plug, &c3, Some(&patch), &tools).is_err());
        assert_eq!(git(&tools, &plug, &["rev-parse", "HEAD"]).unwrap(), c2);
        assert_eq!(
            fs::read_to_string(plug.join("f.txt")).unwrap(),
            "two\nkeep\n"
        );

        // A checkout that fails once the patch is off: put back, patch included.
        let missing = "0123456789abcdef0123456789abcdef01234567";
        assert!(plugin_move(&plug, missing, Some(&patch), &tools).is_err());
        assert_eq!(git(&tools, &plug, &["rev-parse", "HEAD"]).unwrap(), c2);
        assert_eq!(
            fs::read_to_string(plug.join("f.txt")).unwrap(),
            "two\nkeep\n"
        );

        // Changed by hand (not the patch): nothing touched.
        fs::write(plug.join("g.txt"), "mine\n").unwrap();
        assert!(plugin_move(&plug, &c2, Some(&patch), &tools).is_err());
        assert_eq!(fs::read_to_string(plug.join("g.txt")).unwrap(), "mine\n");
        sh(&plug, &["checkout", "--", "g.txt"]);

        // The repository's patch changed: the plugin carries the earlier one (the copy), which
        // is taken off, and the new one put on, at the same commit.
        fs::write(
            &file,
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-one\n+three\n keep\n",
        )
        .unwrap();
        assert!(patch.stale(patch.on(&plug, &tools).as_ref()));
        assert_eq!(patch.on(&plug, &tools), Some(patch.applied.clone()));
        plugin_move(&plug, &c2, Some(&patch), &tools).unwrap();
        assert_eq!(
            fs::read_to_string(plug.join("f.txt")).unwrap(),
            "three\nkeep\n"
        );
        assert!(!plug.join("n.txt").exists());
        assert!(patch.recorded());
        assert!(!patch.stale(patch.on(&plug, &tools).as_ref()));

        // The patch grown on the plugin itself, then written to the repository: the copy still
        // reverses, but the larger file is the one on it.
        fs::write(plug.join("z.txt"), "z\n").unwrap();
        fs::write(
            &file,
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-one\n+three\n keep\n\
             diff --git a/z.txt b/z.txt\nnew file mode 100644\n--- /dev/null\n+++ b/z.txt\n@@ -0,0 +1 @@\n+z\n",
        )
        .unwrap();
        assert_eq!(patch.on(&plug, &tools), Some(file.clone()));
        assert!(!patch.stale(patch.on(&plug, &tools).as_ref()));
        plugin_move(&plug, &c2, Some(&patch), &tools).unwrap();
        assert!(patch.recorded());
        fs::write(
            &file,
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-one\n+three\n keep\n",
        )
        .unwrap();
        plugin_move(&plug, &c2, Some(&patch), &tools).unwrap();
        assert!(!plug.join("z.txt").exists());

        // A new patch that does not apply: the plugin keeps the one it had.
        fs::write(
            &file,
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-nope\n+four\n keep\n",
        )
        .unwrap();
        assert!(plugin_move(&plug, &c2, Some(&patch), &tools).is_err());
        assert_eq!(
            fs::read_to_string(plug.join("f.txt")).unwrap(),
            "three\nkeep\n"
        );
        assert!(patch.stale(patch.on(&plug, &tools).as_ref()));
    }
}
