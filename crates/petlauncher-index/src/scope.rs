//! The scope of the index: the roots, the ignore entries and the home
//! directory. `docs/file-search.md` gives the rules.

use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use globset::{Glob, GlobSet, GlobSetBuilder};

/// What the index contains. A stored index is valid only for an equal scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    /// The directories to scan: `~`, a path that starts with `~/`, or an
    /// absolute path. Other text is relative to the home directory.
    pub roots: Vec<String>,
    /// The ignore entries. [`IgnoreEntry::parse`] gives the three forms.
    pub ignore: Vec<String>,
    /// The directory that `~` means.
    pub home: PathBuf,
    /// Obey the ignore files outside the roots, as `rg` does: the global git
    /// excludes file, and the ignore files in the parent directories of the
    /// roots. The program sets true. A test sets false, so that it does not
    /// read the git settings of the owner.
    pub outside_ignore_files: bool,
}

impl Scope {
    /// A scope that obeys the ignore files outside the roots.
    pub fn new(roots: Vec<String>, ignore: Vec<String>, home: PathBuf) -> Self {
        Self {
            roots,
            ignore,
            home,
            outside_ignore_files: true,
        }
    }

    pub(crate) fn resolve(&self) -> Result<Resolved, ScopeError> {
        let roots = self
            .roots
            .iter()
            .map(|root| {
                let root = root.trim();
                if root.is_empty() {
                    return Err(ScopeError::EmptyRoot);
                }
                Ok(expand(root, &self.home))
            })
            .collect::<Result<_, _>>()?;
        Ok(Resolved {
            roots,
            rules: Arc::new(IgnoreRules::new(&self.ignore, &self.home)?),
            parent_ignore_files: self.outside_ignore_files,
            git_global_excludes: self.outside_ignore_files,
        })
    }
}

/// An error in a root or an ignore entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeError {
    EmptyRoot,
    EmptyIgnoreEntry,
    BadPattern { entry: String, message: String },
}

impl fmt::Display for ScopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRoot => f.write_str("a directory of the file search is empty"),
            Self::EmptyIgnoreEntry => f.write_str("an ignore entry is empty"),
            Self::BadPattern { entry, message } => {
                write!(
                    f,
                    "the ignore pattern \"{entry}\" is not correct: {message}"
                )
            }
        }
    }
}

impl std::error::Error for ScopeError {}

/// One entry of the ignore list, in one of three forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IgnoreEntry {
    /// An absolute path. It matches only that file or directory.
    Path(PathBuf),
    /// A directory name. It matches each directory with that name.
    Name(String),
    /// A glob pattern. It matches each directory whose name matches it.
    Pattern(String),
}

impl IgnoreEntry {
    /// Tells the form of an entry:
    ///
    /// 1. Remove the spaces at the two ends and the slashes at the end.
    /// 2. `~`, or text with a `/`, is a path. `~/` is the home directory.
    ///    A path that does not start with `/` or `~/` is relative to the home
    ///    directory.
    /// 3. Text with `*`, `?` or `[` is a pattern.
    /// 4. All other text is a directory name.
    pub fn parse(text: &str, home: &Path) -> Result<Self, ScopeError> {
        let text = text.trim();
        let text = match text.trim_end_matches('/') {
            "" if text.starts_with('/') => "/",
            stripped => stripped,
        };
        if text.is_empty() {
            Err(ScopeError::EmptyIgnoreEntry)
        } else if text == "~" || text.contains('/') {
            Ok(Self::Path(expand(text, home)))
        } else if text.contains(['*', '?', '[']) {
            glob(text)?;
            Ok(Self::Pattern(text.to_owned()))
        } else {
            Ok(Self::Name(text.to_owned()))
        }
    }
}

fn expand(text: &str, home: &Path) -> PathBuf {
    let path = match text.strip_prefix('~') {
        Some("") => home.to_path_buf(),
        Some(rest) if rest.starts_with('/') => home.join(rest.trim_start_matches('/')),
        // `join` with an absolute path gives that path.
        _ => home.join(text),
    };
    path.components().collect()
}

fn glob(text: &str) -> Result<Glob, ScopeError> {
    Glob::new(text).map_err(|error| ScopeError::BadPattern {
        entry: text.to_owned(),
        message: error.kind().to_string(),
    })
}

/// The scope after the expansion of `~` and the parse of the ignore entries.
pub(crate) struct Resolved {
    pub roots: Vec<PathBuf>,
    pub rules: Arc<IgnoreRules>,
    pub parent_ignore_files: bool,
    pub git_global_excludes: bool,
}

pub(crate) struct IgnoreRules {
    paths: HashSet<PathBuf>,
    names: HashSet<OsString>,
    patterns: GlobSet,
}

impl IgnoreRules {
    pub fn new(entries: &[String], home: &Path) -> Result<Self, ScopeError> {
        let mut paths = HashSet::new();
        let mut names = HashSet::new();
        let mut patterns = GlobSetBuilder::new();
        for entry in entries {
            match IgnoreEntry::parse(entry, home)? {
                IgnoreEntry::Path(path) => {
                    paths.insert(path);
                }
                IgnoreEntry::Name(name) => {
                    names.insert(OsString::from(name));
                }
                IgnoreEntry::Pattern(pattern) => {
                    patterns.add(glob(&pattern)?);
                }
            }
        }
        let patterns = patterns.build().map_err(|error| ScopeError::BadPattern {
            entry: error.glob().unwrap_or_default().to_owned(),
            message: error.kind().to_string(),
        })?;
        Ok(Self {
            paths,
            names,
            patterns,
        })
    }

    pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
        if self.paths.contains(path) {
            return true;
        }
        if !is_dir {
            return false;
        }
        path.file_name()
            .is_some_and(|name| self.names.contains(name) || self.patterns.is_match(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<IgnoreEntry, ScopeError> {
        IgnoreEntry::parse(text, Path::new("/home/owner"))
    }

    fn path(text: &str) -> Result<IgnoreEntry, ScopeError> {
        Ok(IgnoreEntry::Path(PathBuf::from(text)))
    }

    #[test]
    fn tells_a_path_from_the_tilde_or_a_slash() {
        assert_eq!(parse("~/tmp"), path("/home/owner/tmp"));
        assert_eq!(parse("~/go/pkg/mod"), path("/home/owner/go/pkg/mod"));
        assert_eq!(parse("~"), path("/home/owner"));
        assert_eq!(parse("/mnt/data"), path("/mnt/data"));
    }

    #[test]
    fn reads_a_relative_path_as_relative_to_the_home_directory() {
        assert_eq!(parse("go/pkg/mod"), path("/home/owner/go/pkg/mod"));
    }

    #[test]
    fn removes_spaces_and_slashes_at_the_end() {
        assert_eq!(parse("  ~/tmp/ "), path("/home/owner/tmp"));
        assert_eq!(parse("~/"), path("/home/owner"));
        assert_eq!(parse("/"), path("/"));
        assert_eq!(
            parse("node_modules/"),
            Ok(IgnoreEntry::Name("node_modules".to_owned()))
        );
    }

    #[test]
    fn tells_a_pattern_from_a_glob_character() {
        for text in ["*-wt", "build?", "[ab]uild", "*.{a,b}"] {
            assert_eq!(parse(text), Ok(IgnoreEntry::Pattern(text.to_owned())));
        }
    }

    #[test]
    fn reads_other_text_as_a_directory_name() {
        for text in [
            "node_modules",
            "target",
            "__pycache__",
            "git-worktrees",
            "~foo",
        ] {
            assert_eq!(parse(text), Ok(IgnoreEntry::Name(text.to_owned())));
        }
    }

    #[test]
    fn rejects_an_empty_entry_and_a_bad_pattern() {
        assert_eq!(parse("  "), Err(ScopeError::EmptyIgnoreEntry));
        assert!(matches!(
            parse("[abc"),
            Err(ScopeError::BadPattern { entry, .. }) if entry == "[abc"
        ));
    }

    #[test]
    fn expands_the_roots_and_rejects_an_empty_root() {
        let scope = |roots: &[&str]| {
            Scope::new(
                roots.iter().map(|r| r.to_string()).collect(),
                vec![],
                PathBuf::from("/home/owner"),
            )
        };

        let resolved = scope(&["~", "~/dev/", "/data", "notes"]).resolve().unwrap();

        assert_eq!(
            resolved.roots,
            [
                "/home/owner",
                "/home/owner/dev",
                "/data",
                "/home/owner/notes"
            ]
            .map(PathBuf::from)
        );
        assert_eq!(scope(&[" "]).resolve().err(), Some(ScopeError::EmptyRoot));
    }

    #[test]
    fn a_path_entry_matches_files_and_directories_but_not_a_longer_name() {
        let rules = IgnoreRules::new(&["~/tmp".to_owned()], Path::new("/h")).unwrap();

        assert!(rules.is_ignored(Path::new("/h/tmp"), true));
        assert!(rules.is_ignored(Path::new("/h/tmp"), false));
        assert!(!rules.is_ignored(Path::new("/h/tmp2"), true));
        assert!(!rules.is_ignored(Path::new("/h/dev/tmp"), true));
    }

    #[test]
    fn name_and_pattern_entries_match_directories_only() {
        let entries = ["target".to_owned(), "*-wt".to_owned()];
        let rules = IgnoreRules::new(&entries, Path::new("/h")).unwrap();

        assert!(rules.is_ignored(Path::new("/h/a/target"), true));
        assert!(rules.is_ignored(Path::new("/h/a/petlauncher-wt"), true));
        assert!(!rules.is_ignored(Path::new("/h/a/target"), false));
        assert!(!rules.is_ignored(Path::new("/h/a/notes-wt"), false));
        assert!(!rules.is_ignored(Path::new("/h/a/targets"), true));
        assert!(!rules.is_ignored(Path::new("/h/a/wt-old"), true));
    }

    #[test]
    fn displays_each_error_with_the_entry() {
        let error = parse("[abc").unwrap_err();

        assert!(error.to_string().starts_with("the ignore pattern \"[abc\""));
        assert!(!ScopeError::EmptyRoot.to_string().is_empty());
        assert!(!ScopeError::EmptyIgnoreEntry.to_string().is_empty());
    }
}
