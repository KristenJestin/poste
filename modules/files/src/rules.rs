//! What gets indexed: walk of the roots with the `ignore` crate (the one of ripgrep and fff),
//! filtered by our own rules only, in .gitignore syntax: the module's `default.ignore`, then
//! `~/.config/poste/files.ignore`. The `.gitignore` and `.ignore` files met are not read. An
//! excluded folder is never opened: `filter_entry` drops it before going down.

use anyhow::{Context, Result};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::{WalkBuilder, WalkState};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Patterns shipped with the module, read before the user's.
pub const DEFAULT_IGNORE: &str = include_str!("../default.ignore");

/// Exclusion rules, one set per root (patterns are anchored on the root).
#[derive(Debug)]
pub struct Rules {
    roots: Vec<(PathBuf, Gitignore)>,
}

impl Rules {
    /// `user`: text of files.ignore (`source`: its name, for errors).
    pub fn new(roots: &[PathBuf], user: &str, source: &str) -> Result<Self> {
        let mut out = Vec::new();
        for root in roots {
            let mut b = GitignoreBuilder::new(root);
            let lines = DEFAULT_IGNORE
                .lines()
                .enumerate()
                .map(|(n, l)| ("default.ignore", n, l))
                .chain(user.lines().enumerate().map(|(n, l)| (source, n, l)));
            for (file, n, line) in lines {
                b.add_line(None, line)
                    .with_context(|| format!("{file}, line {}: invalid pattern", n + 1))?;
            }
            out.push((root.clone(), b.build().context("exclusion rules")?));
        }
        Ok(Self { roots: out })
    }

    /// `path` is excluded. Only looks at `path` itself: its parents were already accepted by
    /// the walk (as in git, nothing comes back under an excluded folder).
    pub fn excluded(&self, path: &Path, is_dir: bool) -> bool {
        self.roots
            .iter()
            .find(|(r, _)| path.starts_with(r) && path != r)
            .is_some_and(|(_, gi)| gi.matched(path, is_dir).is_ignore())
    }
}

/// A folder, or a symbolic link to a folder: it counts as a folder (for patterns ending in
/// "/" and in the index), without being followed.
fn is_dir(e: &ignore::DirEntry) -> bool {
    e.file_type()
        .is_some_and(|t| t.is_dir() || (t.is_symlink() && e.path().is_dir()))
}

/// Walks `root` (a root of `rules`); returns the files and folders found (absolute path,
/// folder or not), without `root` itself. `threads`: walk threads.
pub fn walk(rules: &Arc<Rules>, root: &Path, threads: usize) -> Vec<(PathBuf, bool)> {
    let mut b = WalkBuilder::new(root);
    b.standard_filters(false)
        .follow_links(false)
        .threads(threads.max(1));
    let r = Arc::clone(rules);
    b.filter_entry(move |e| e.depth() == 0 || !r.excluded(e.path(), is_dir(e)));
    let out = Mutex::new(Vec::new());
    b.build_parallel().run(|| {
        let out = &out;
        Box::new(move |res| {
            let Ok(e) = res else {
                return WalkState::Continue;
            };
            if e.depth() > 0 && e.file_type().is_some() {
                let d = is_dir(&e);
                out.lock().unwrap().push((e.into_path(), d));
            }
            WalkState::Continue
        })
    });
    out.into_inner().unwrap()
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::fs;

    /// Test tree: files to find and everything that must be left out.
    pub fn sample_tree(root: &Path) {
        let files = [
            "notes/pancake-recipe.md",
            "Projects/site/src/main.rs",
            "Projects/site/out/bundle.js",
            "Projects/site/kept.log",
            "Projects/site/node_modules/left-pad/index.js",
            "Projects/game/Library/ShaderCache/a.bin",
            "Projects/game/Assets/Player.cs",
            "Projects/py/__pycache__/m.cpython.pyc",
            "Projects/py/.venv/bin/python",
            "Projects/py/app.py",
            "Projects/web/.next/cache.json",
            "Projects/web/.svelte-kit/x.js",
            "Projects/web/dist/index.html",
            "Projects/web/build/index.html",
            "Projects/web/node/bin/node",
            "Projects/rs/target/debug/rs",
            ".cache/thumb.png",
            ".config/app/reglages.toml",
            "Pictures/holiday.jpg",
        ];
        for f in files {
            let p = root.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, "x").unwrap();
        }
        fs::create_dir_all(root.join("Projects/site/.git")).unwrap();
        fs::write(root.join("Projects/site/.git/HEAD"), "ref: x").unwrap();
        // Link named like an excluded folder: excluded too.
        fs::create_dir_all(root.join("Projects/shared")).unwrap();
        std::os::unix::fs::symlink(
            root.join("Projects/shared"),
            root.join("Projects/py/node_modules"),
        )
        .unwrap();
        // Ignored by git, but we want to find them: .gitignore files are not read.
        fs::write(root.join("Projects/site/.gitignore"), "out/\n*.log\n").unwrap();
    }

    fn rel(root: &Path, w: &[(PathBuf, bool)]) -> Vec<String> {
        let mut e: Vec<_> = w
            .iter()
            .map(|(p, _)| p.strip_prefix(root).unwrap().to_string_lossy().into_owned())
            .collect();
        e.sort();
        e
    }

    pub fn rules(root: &Path, user: &str) -> Arc<Rules> {
        Arc::new(Rules::new(&[root.to_path_buf()], user, "files.ignore").unwrap())
    }

    #[test]
    fn default_exclusions_and_gitignore_not_read() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("home");
        sample_tree(&root);
        let entries = rel(&root, &walk(&rules(&root, ""), &root, 2));
        for bad in [
            "node_modules",
            "Library",
            "target",
            ".git",
            ".cache",
            "dist",
            "build",
            "node",
            ".venv",
            "__pycache__",
            ".next",
            ".svelte-kit",
            ".config",
            ".gitignore",
        ] {
            let hit = |s: &String| s.split('/').any(|c| c == bad);
            assert!(!entries.iter().any(hit), "{bad} indexed: {entries:?}");
        }
        for good in [
            "notes/pancake-recipe.md",
            "Projects/site/src/main.rs",
            "Projects/site/out/bundle.js",
            "Projects/site/kept.log",
            "Projects/game/Assets/Player.cs",
            "Projects/py/app.py",
            "Pictures/holiday.jpg",
            "Projects",
        ] {
            assert!(entries.iter().any(|e| e == good), "{good} missing");
        }
    }

    #[test]
    fn user_file_adds_and_lifts() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("home");
        sample_tree(&root);
        fs::create_dir_all(root.join("Archives/build")).unwrap();
        fs::write(root.join("Archives/build/vieux.zip"), "x").unwrap();
        let user = "\
# commentaire
Pictures/
*.md
!build/
!.config/
/Projects/py/
";
        let entries = rel(&root, &walk(&rules(&root, user), &root, 1));
        // Added: a folder, a name pattern, a path anchored on the root.
        assert!(!entries.iter().any(|e| e.starts_with("Pictures")));
        assert!(!entries.iter().any(|e| e.ends_with(".md")));
        assert!(!entries.iter().any(|e| e.starts_with("Projects/py")));
        // Lifted: build/ and .config/ come back, the other exclusions hold.
        assert!(entries.iter().any(|e| e == "Projects/web/build/index.html"));
        assert!(entries.iter().any(|e| e == "Archives/build/vieux.zip"));
        assert!(entries.iter().any(|e| e == ".config/app/reglages.toml"));
        assert!(!entries.iter().any(|e| e.starts_with(".cache")));
        assert!(!entries.iter().any(|e| e.contains(".git/")));
        assert!(!entries.iter().any(|e| e.contains("node_modules")));
    }

    #[test]
    fn anchored_on_the_root_only() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("home");
        sample_tree(&root);
        fs::create_dir_all(root.join("notes/Pictures")).unwrap();
        fs::write(root.join("notes/Pictures/schema.png"), "x").unwrap();
        let entries = rel(&root, &walk(&rules(&root, "/Pictures/\n"), &root, 1));
        assert!(!entries.iter().any(|e| e.starts_with("Pictures")));
        assert!(entries.iter().any(|e| e == "notes/Pictures/schema.png"));
    }

    #[test]
    fn invalid_pattern_reported_with_its_line() {
        let e = Rules::new(&[PathBuf::from("/h")], "ok/\n[z-a]\n", "files.ignore").unwrap_err();
        assert!(format!("{e:#}").contains("files.ignore, line 2"), "{e:#}");
    }
}
