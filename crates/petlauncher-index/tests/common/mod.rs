//! Generated directory trees for the tests. Each tree is a new directory below
//! `TMPDIR`, which is `$HOME/tmp` when the tests run as the plan tells. The
//! tests never read the real home directory of the owner.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

use petlauncher_index::{scan, Index, Scope};
use tempfile::TempDir;

/// The initial ignore list of R14, as `petlauncher-config` gives it.
pub const INITIAL_IGNORE: [&str; 7] = [
    "node_modules",
    "target",
    "__pycache__",
    "~/go/pkg/mod",
    "~/tmp",
    "git-worktrees",
    "*-wt",
];

pub struct Tree {
    dir: TempDir,
}

impl Tree {
    pub fn new() -> Self {
        let dir = tempfile::Builder::new()
            .prefix("petlauncher-index-test-")
            .tempdir()
            .expect("make a temporary directory");
        Self { dir }
    }

    /// The directory that the tests use as the home directory.
    pub fn home(&self) -> &Path {
        self.dir.path()
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.home().join(relative)
    }

    pub fn file(&self, relative: &str) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, relative).unwrap();
        path
    }

    pub fn files(&self, relatives: &[&str]) {
        for relative in relatives {
            self.file(relative);
        }
    }

    pub fn dir(&self, relative: &str) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(&path).unwrap();
        path
    }

    pub fn symlink(&self, target: &str, link: &str) {
        std::os::unix::fs::symlink(self.path(target), self.path(link)).unwrap();
    }

    /// Runs a real `git init`, with no git settings of the owner.
    pub fn git_init(&self, relative: &str) {
        let dir = self.dir(relative);
        let status = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&dir)
            .env("HOME", self.home())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .expect("run git");
        assert!(status.success(), "git init failed in {}", dir.display());
    }

    /// A scope with the root `~` and no ignore files outside the root.
    pub fn scope(&self, ignore: &[&str]) -> Scope {
        Scope {
            roots: vec!["~".to_owned()],
            ignore: ignore.iter().map(|e| e.to_string()).collect(),
            home: self.home().to_path_buf(),
            outside_ignore_files: false,
        }
    }

    pub fn scan(&self, scope: &Scope) -> Index {
        scan(scope, 2, &AtomicBool::new(false)).expect("scan")
    }

    /// The paths in the index, relative to the home directory. A directory
    /// ends with a slash.
    pub fn relative_paths(&self, index: &Index) -> BTreeSet<String> {
        index
            .iter()
            .map(|entry| {
                let relative = entry.path.strip_prefix(self.home()).unwrap();
                let slash = if entry.is_dir { "/" } else { "" };
                format!("{}{slash}", relative.display())
            })
            .collect()
    }

    /// Scans with `ignore` and gives the relative paths.
    pub fn scan_paths(&self, ignore: &[&str]) -> BTreeSet<String> {
        self.relative_paths(&self.scan(&self.scope(ignore)))
    }
}

pub fn set(paths: &[&str]) -> BTreeSet<String> {
    paths.iter().map(|p| p.to_string()).collect()
}
