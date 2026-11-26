//! The scan of the roots.

use std::fmt;
use std::mem;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use ignore::{DirEntry, ParallelVisitor, ParallelVisitorBuilder, WalkBuilder, WalkState};

use crate::index::{FileEntry, Index};
use crate::scope::{Resolved, Scope, ScopeError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanError {
    Scope(ScopeError),
    Cancelled,
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scope(error) => error.fmt(f),
            Self::Cancelled => f.write_str("the scan was cancelled"),
        }
    }
}

impl std::error::Error for ScanError {}

/// Scans the roots of `scope` and makes an index.
///
/// `threads` is the number of threads. 0 lets the `ignore` crate select it.
/// The scan stops soon after `cancel` becomes true.
pub fn scan(scope: &Scope, threads: usize, cancel: &AtomicBool) -> Result<Index, ScanError> {
    let resolved = scope.resolve().map_err(ScanError::Scope)?;
    scan_resolved(&resolved, threads, cancel).ok_or(ScanError::Cancelled)
}

/// Returns `None` when the scan was cancelled.
pub(crate) fn scan_resolved(
    scope: &Resolved,
    threads: usize,
    cancel: &AtomicBool,
) -> Option<Index> {
    let started = SystemTime::now();
    let Some((first, others)) = scope.roots.split_first() else {
        return Some(Index::build(Vec::new(), started, 0));
    };
    let mut walk = WalkBuilder::new(first);
    for root in others {
        walk.add(root);
    }
    let rules = Arc::clone(&scope.rules);
    walk.threads(threads)
        .follow_links(false)
        .hidden(true)
        .parents(scope.parent_ignore_files)
        .git_global(scope.git_global_excludes)
        .filter_entry(move |entry| !rules.is_ignored(entry.path(), is_dir(entry)));

    let found = Mutex::new(Vec::new());
    let errors = AtomicU32::new(0);
    walk.build_parallel().visit(&mut Collector {
        cancel,
        found: &found,
        errors: &errors,
    });

    if cancel.load(Ordering::Relaxed) {
        return None;
    }
    let found: Vec<Vec<FileEntry>> = found.into_inner().unwrap_or_else(|e| e.into_inner());
    Some(Index::build(found.concat(), started, errors.into_inner()))
}

fn is_dir(entry: &DirEntry) -> bool {
    entry.file_type().is_some_and(|t| t.is_dir())
}

fn is_file_dir_or_link(entry: &DirEntry) -> bool {
    entry
        .file_type()
        .is_some_and(|t| t.is_file() || t.is_dir() || t.is_symlink())
}

/// Gives each thread of the walk a [`Visitor`] with its own list. The
/// visitor adds its list to `found` when the thread stops.
struct Collector<'s> {
    cancel: &'s AtomicBool,
    found: &'s Mutex<Vec<Vec<FileEntry>>>,
    errors: &'s AtomicU32,
}

impl<'s> ParallelVisitorBuilder<'s> for Collector<'s> {
    fn build(&mut self) -> Box<dyn ParallelVisitor + 's> {
        Box::new(Visitor {
            cancel: self.cancel,
            found: self.found,
            errors: self.errors,
            found_here: Vec::new(),
        })
    }
}

struct Visitor<'s> {
    cancel: &'s AtomicBool,
    found: &'s Mutex<Vec<Vec<FileEntry>>>,
    errors: &'s AtomicU32,
    found_here: Vec<FileEntry>,
}

impl ParallelVisitor for Visitor<'_> {
    fn visit(&mut self, entry: Result<DirEntry, ignore::Error>) -> WalkState {
        if self.cancel.load(Ordering::Relaxed) {
            return WalkState::Quit;
        }
        match entry {
            Ok(entry) if entry.depth() > 0 && is_file_dir_or_link(&entry) => {
                let modified = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(UNIX_EPOCH);
                self.found_here.push(FileEntry {
                    is_dir: is_dir(&entry),
                    path: entry.into_path(),
                    modified,
                });
            }
            // A root, a socket, a pipe or a device.
            Ok(_) => {}
            Err(_) => {
                self.errors.fetch_add(1, Ordering::Relaxed);
            }
        }
        WalkState::Continue
    }
}

impl Drop for Visitor<'_> {
    fn drop(&mut self) {
        let found = mem::take(&mut self.found_here);
        self.found
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(found);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use super::*;
    use crate::scope::IgnoreRules;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// The test turns on only the global git excludes, and gives them with
    /// `GIT_CONFIG_GLOBAL`, so it reads no git settings of the owner. No other
    /// test in this binary uses the global excludes.
    #[test]
    fn obeys_the_global_git_excludes_inside_a_git_repository_only() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let excludes = home.join("config/git-excludes");
        let config = home.join("config/gitconfig");
        write(&excludes, "*.bak\n");
        write(
            &config,
            &format!("[core]\n\texcludesFile = {}\n", excludes.display()),
        );
        write(&home.join("repo/keep.txt"), "");
        write(&home.join("repo/old.bak"), "");
        write(&home.join("outside/old.bak"), "");
        let status = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(home.join("repo"))
            .env("HOME", home)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap();
        assert!(status.success());
        std::env::set_var("GIT_CONFIG_GLOBAL", &config);
        let scope = Resolved {
            roots: vec![home.to_path_buf()],
            rules: Arc::new(IgnoreRules::new(&[], home).unwrap()),
            parent_ignore_files: false,
            git_global_excludes: true,
        };

        let index = scan_resolved(&scope, 2, &AtomicBool::new(false)).unwrap();

        let paths: Vec<_> = index
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.path.strip_prefix(home).unwrap().to_path_buf())
            .collect();
        assert!(paths.contains(&"repo/keep.txt".into()));
        assert!(!paths.contains(&"repo/old.bak".into()));
        assert!(paths.contains(&"outside/old.bak".into()));
    }
}
