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

/// Tag of the release to follow.
pub fn latest_tag(app: &App, tools: &Tools) -> Result<String> {
    let tag = if app.prerelease {
        output(
            &tools.gh,
            &[
                "release",
                "list",
                "-R",
                &app.github,
                "--exclude-drafts",
                "--limit",
                "1",
                "--json",
                "tagName",
                "--jq",
                ".[0].tagName",
            ],
        )?
    } else {
        output(
            &tools.gh,
            &[
                "release",
                "view",
                "-R",
                &app.github,
                "--json",
                "tagName",
                "--jq",
                ".tagName",
            ],
        )?
    };
    if tag.is_empty() || tag == "null" {
        bail!("{}: no release published", app.github);
    }
    Ok(tag)
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

/// The asset of the release matching `asset`, and its sha256 published by GitHub.
pub fn pick_asset(app: &App, tag: &str, listing: &str) -> Result<(String, String)> {
    let pat = app.asset.replace("{version}", &norm(tag));
    let found: Vec<(&str, &str)> = listing
        .lines()
        .filter_map(|l| l.trim().split_once(' '))
        .filter(|(name, _)| glob(&pat, name))
        .collect();
    match found.as_slice() {
        [(name, digest)] => {
            let Some(sum) = digest.strip_prefix("sha256:") else {
                bail!("{tag}: no sha256 published by GitHub for {name}, nothing installed");
            };
            Ok((name.to_string(), sum.to_string()))
        }
        [] => bail!("{tag}: no asset {pat} in the release"),
        _ => bail!("{tag}: several assets match {pat}"),
    }
}

fn sha256_file(p: &Path) -> Result<String> {
    let mut f = std::fs::File::open(p).with_context(|| format!("reading {}", p.display()))?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h)?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Downloads the asset of `tag` into `dir` and checks its sha256; returns its path.
pub fn download(app: &App, tag: &str, dir: &Path, tools: &Tools) -> Result<PathBuf> {
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
    let (name, sum) = pick_asset(app, tag, &listing)?;
    std::fs::create_dir_all(dir)?;
    let file = dir.join(&name);
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
            &name,
            "-D",
            &d,
            "--clobber",
        ],
    )?;
    let got = sha256_file(&file)?;
    if got != sum {
        let _ = std::fs::remove_file(&file);
        bail!("{name}: wrong sha256 ({got}, expected {sum}), removed, nothing installed");
    }
    Ok(file)
}

/// What an installation left to do by hand (a pacman package: the `sudo` command).
pub struct Installed {
    pub tag: String,
    pub by_hand: Option<String>,
}

/// Installs the release `tag` (the latest if `None`). `busy` and paths already substituted.
/// `cache` keeps the packages to install by hand; the other archives go through a temporary
/// folder in it, removed after.
pub fn install(
    app: &App,
    tag: Option<&str>,
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
    let tag = match tag {
        Some(t) => t.to_string(),
        None => latest_tag(app, tools)?,
    };
    if let Install::Package(_) = how {
        let file = download(app, &tag, &cache.join("apps"), tools)?;
        return Ok(Installed {
            by_hand: Some(format!("sudo pacman -U {}", file.display())),
            tag,
        });
    }
    let tmp = cache.join(format!("apps/.{}-{}", app.id, std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let res = (|| {
        let archive = download(app, &tag, &tmp, tools)?;
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
                let src = x.join(b);
                if !src.is_file() {
                    bail!("{b}: not in the archive");
                }
                let name = Path::new(b).file_name().context("empty bin")?;
                std::fs::create_dir_all(bin_dir)?;
                let new = bin_dir.join(format!(".{}.new", name.to_string_lossy()));
                std::fs::copy(&src, &new)?;
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&new, std::fs::Permissions::from_mode(0o755))?;
                std::fs::rename(&new, bin_dir.join(name))?;
            }
            Install::Script(s) => {
                let script = x.join(s.replace("{version}", &norm(&tag)));
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
    res.map(|()| Installed { tag, by_hand: None })
}

// ------------------------------------------------------------------ plugins

/// Target of a plugin without a pinned commit: its most recent version tag if it has tags,
/// otherwise the head of its default branch. From `git ls-remote --tags --sort=-v:refname
/// origin` then `git ls-remote origin HEAD`: (name, commit).
pub fn plugin_target(tags: &str, head: &str) -> Option<(String, String)> {
    // A release first (no `-rc1`, `-beta` suffix), a pre-release only if there is nothing else.
    first_tag(tags, false)
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

pub fn plugin_state(dir: &Path, tools: &Tools) -> Result<PluginState> {
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
    let target = plugin_target(&tags, &remote_head);
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

/// Moves a plugin to `commit`, its local patch taken off then put back. If the patch no longer
/// applies, the plugin is put back where it was, patch included, and the update fails.
pub fn plugin_move(dir: &Path, commit: &str, patch: Option<&Path>, tools: &Tools) -> Result<()> {
    // The network first: once the patch is off, nothing should be left that can fail alone.
    git(tools, dir, &["fetch", "--quiet", "--tags", "origin"])?;
    let old = git(tools, dir, &["rev-parse", "HEAD"])?;
    let p = patch.map(|p| p.to_string_lossy().into_owned());
    let status = |tools: &Tools| {
        git(
            tools,
            dir,
            &["status", "--porcelain", "--untracked-files=no"],
        )
    };
    let mut unpatched = false;
    if !status(tools)?.is_empty() {
        match &p {
            Some(p) if git(tools, dir, &["apply", "--reverse", "--check", p]).is_ok() => {
                git(tools, dir, &["apply", "--reverse", p])?;
                unpatched = true;
                // A file the patch created and that was marked `git add -N`: its empty index
                // entry stays once the file is gone, and is only a placeholder.
                let summary = git(tools, dir, &["apply", "--summary", p])?;
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
                    git(tools, dir, &["apply", p])?;
                    bail!(
                        "{}: files modified by hand besides the patch, nothing touched",
                        dir.display()
                    );
                }
            }
            _ => bail!("{}: files modified by hand, nothing touched", dir.display()),
        }
    }
    // From here on, any failure puts the plugin back where it was, patch included.
    let back = |why: anyhow::Error| -> Result<()> {
        git(tools, dir, &["checkout", "--quiet", "--detach", &old])
            .and_then(|_| match (&p, unpatched) {
                (Some(p), true) => git(tools, dir, &["apply", p]).map(|_| ()),
                _ => Ok(()),
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
    if let Err(e) = git(tools, dir, &["checkout", "--quiet", "--detach", commit]) {
        return back(e);
    }
    if let Some(p) = &p {
        if git(tools, dir, &["apply", "--check", p]).is_err() {
            return back(anyhow::anyhow!(
                "the patch {p} no longer applies to {}: plugin left where it was, update the patch",
                &commit[..commit.len().min(7)]
            ));
        }
        if let Err(e) = git(tools, dir, &["apply", p]) {
            return back(e);
        }
    }
    Ok(())
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
            ("v-0.2.0-1-x86_64.pkg.tar.zst".into(), "abc".into())
        );
        assert!(pick_asset(&a, "v0.3.0", listing).is_err(), "absent");
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
            plugin_target(tags, "fff\tHEAD"),
            Some(("v1.2.0".into(), "bbb".into()))
        );
        let light = "ddd\trefs/tags/v2.0.0\nccc\trefs/tags/v1.1.0\n";
        assert_eq!(
            plugin_target(light, "fff\tHEAD"),
            Some(("v2.0.0".into(), "ddd".into()))
        );
        assert_eq!(
            plugin_target("", "fff\tHEAD"),
            Some(("default branch".into(), "fff".into()))
        );
        assert_eq!(plugin_target("", ""), None);
        // A release before a pre-release; a pre-release only when there is nothing else.
        let rc = "eee\trefs/tags/v3.0.0-rc1\nddd\trefs/tags/v2.0.0\n";
        assert_eq!(plugin_target(rc, ""), Some(("v2.0.0".into(), "ddd".into())));
        assert_eq!(
            plugin_target("eee\trefs/tags/v3.0.0-rc1\n", ""),
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
                 *'--json tagName'*) echo '{tag}' ;;\n\
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
        assert_eq!(done.tag, "v1.2.0");
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
            done.by_hand.unwrap(),
            format!("sudo pacman -U {}", kept.display())
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
        let patch = d.path().join("p.patch");
        fs::write(
            &patch,
            "diff --git a/f.txt b/f.txt\n--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n-one\n+two\n keep\n\
             diff --git a/n.txt b/n.txt\nnew file mode 100644\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1 @@\n+new\n",
        )
        .unwrap();
        sh(&plug, &["apply", &patch.to_string_lossy()]);
        // The created file marked intent-to-add, as found on a real plugin.
        sh(&plug, &["add", "-N", "n.txt"]);
        let tools = Tools::default();

        // Upstream changes another line: the patch follows.
        fs::write(up.join("g.txt"), "new\n").unwrap();
        sh(&up, &["add", "."]);
        sh(&up, &["commit", "-q", "-m", "2"]);
        sh(&up, &["tag", "v2"]);
        let st = plugin_state(&plug, &tools).unwrap();
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
    }
}
