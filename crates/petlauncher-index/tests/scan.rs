//! The scan of generated trees: the ignore list of R14, hidden entries,
//! `.gitignore`, symbolic links and roots.

mod common;

use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::{set, Tree, INITIAL_IGNORE};
use petlauncher_index::{scan, ScanError, Scope, ScopeError};

#[test]
fn skips_directories_named_node_modules_target_and_pycache_at_any_depth() {
    let tree = Tree::new();
    tree.files(&[
        "web/node_modules/react/index.js",
        "web/src/node_modules/deep/x.js",
        "rust/target/debug/app",
        "py/pkg/__pycache__/mod.cpython-312.pyc",
        "py/pkg/mod.py",
    ]);

    let paths = tree.scan_paths(&INITIAL_IGNORE);

    assert_eq!(
        paths,
        set(&[
            "web/",
            "web/src/",
            "rust/",
            "py/",
            "py/pkg/",
            "py/pkg/mod.py"
        ])
    );
}

#[test]
fn skips_the_go_module_cache_of_the_home_directory_only() {
    let tree = Tree::new();
    tree.files(&[
        "go/pkg/mod/github.com/x/y.go",
        "go/src/app/main.go",
        "dev/vendor/go/pkg/mod/kept.go",
    ]);

    let paths = tree.scan_paths(&INITIAL_IGNORE);

    assert!(paths.contains("go/pkg/"));
    assert!(!paths.contains("go/pkg/mod/"));
    assert!(!paths.iter().any(|p| p.starts_with("go/pkg/mod")));
    assert!(paths.contains("go/src/app/main.go"));
    assert!(paths.contains("dev/vendor/go/pkg/mod/kept.go"));
}

#[test]
fn skips_the_tmp_directory_of_the_home_directory_only() {
    let tree = Tree::new();
    tree.files(&["tmp/cargo-target/debug/x", "tmp2/kept", "dev/tmp/kept"]);

    let paths = tree.scan_paths(&INITIAL_IGNORE);

    assert_eq!(
        paths,
        set(&["tmp2/", "tmp2/kept", "dev/", "dev/tmp/", "dev/tmp/kept"])
    );
}

#[test]
fn skips_extra_git_worktrees_and_keeps_the_main_checkout() {
    let tree = Tree::new();
    tree.files(&[
        "dev/petlauncher/README.md",
        "dev/petlauncher-wt/index/README.md",
        "dev/git-worktrees/feature/README.md",
        "dev/wt/kept.md",
    ]);

    let paths = tree.scan_paths(&INITIAL_IGNORE);

    assert_eq!(
        paths,
        set(&[
            "dev/",
            "dev/petlauncher/",
            "dev/petlauncher/README.md",
            "dev/wt/",
            "dev/wt/kept.md",
        ])
    );
}

#[test]
fn name_and_pattern_entries_do_not_skip_files() {
    let tree = Tree::new();
    tree.files(&["notes/target", "notes/meeting-wt", "notes/node_modules"]);

    let paths = tree.scan_paths(&INITIAL_IGNORE);

    assert_eq!(
        paths,
        set(&[
            "notes/",
            "notes/target",
            "notes/meeting-wt",
            "notes/node_modules"
        ])
    );
}

#[test]
fn an_added_path_entry_removes_that_directory() {
    let tree = Tree::new();
    tree.files(&["lolcat-evidence/a.png", "lolcat/b.md"]);

    let paths = tree.scan_paths(&["~/lolcat-evidence"]);

    assert_eq!(paths, set(&["lolcat/", "lolcat/b.md"]));
}

#[test]
fn skips_hidden_files_and_directories() {
    let tree = Tree::new();
    tree.files(&[
        ".config/app/settings.toml",
        ".bashrc",
        "dev/.cache/x",
        "dev/kept",
    ]);

    let paths = tree.scan_paths(&[]);

    assert_eq!(paths, set(&["dev/", "dev/kept"]));
}

#[test]
fn obeys_gitignore_and_git_exclude_inside_a_git_repository() {
    let tree = Tree::new();
    tree.git_init("dev/repo");
    tree.files(&[
        "dev/repo/src/main.rs",
        "dev/repo/debug.log",
        "dev/repo/build/out.bin",
        "dev/repo/secret.md",
    ]);
    fs::write(tree.path("dev/repo/.gitignore"), "*.log\nbuild/\n").unwrap();
    fs::write(tree.path("dev/repo/.git/info/exclude"), "secret.md\n").unwrap();

    let paths = tree.scan_paths(&[]);

    assert_eq!(
        paths,
        set(&["dev/", "dev/repo/", "dev/repo/src/", "dev/repo/src/main.rs"])
    );
}

#[test]
fn does_not_obey_gitignore_outside_a_git_repository() {
    let tree = Tree::new();
    tree.files(&["notes/todo.txt"]);
    fs::write(tree.path("notes/.gitignore"), "*.txt\n").unwrap();

    let paths = tree.scan_paths(&[]);

    assert_eq!(paths, set(&["notes/", "notes/todo.txt"]));
}

#[test]
fn lists_a_symbolic_link_but_does_not_follow_it() {
    let tree = Tree::new();
    tree.files(&["real/inner.txt"]);
    tree.symlink("real", "link");
    tree.symlink("", "loop");

    let paths = tree.scan_paths(&[]);

    assert_eq!(paths, set(&["real/", "real/inner.txt", "link", "loop"]));
}

#[test]
fn obeys_an_ignore_file_also_outside_a_git_repository() {
    let tree = Tree::new();
    tree.files(&["notes/a.tmp", "notes/b.md"]);
    fs::write(tree.path("notes/.ignore"), "*.tmp\n").unwrap();

    let paths = tree.scan_paths(&[]);

    assert_eq!(paths, set(&["notes/", "notes/b.md"]));
}

#[test]
fn scans_a_root_that_is_a_symbolic_link_to_a_directory() {
    let tree = Tree::new();
    tree.files(&["real/x"]);
    tree.symlink("real", "link");
    let mut scope = tree.scope(&[]);
    scope.roots = vec!["~/link".into()];

    let index = tree.scan(&scope);

    assert_eq!(tree.relative_paths(&index), set(&["link/x"]));
}

#[test]
fn leaves_out_a_socket() {
    let tree = Tree::new();
    tree.dir("run");
    let _socket = std::os::unix::net::UnixListener::bind(tree.path("run/app.sock")).unwrap();

    let paths = tree.scan_paths(&[]);

    assert_eq!(paths, set(&["run/"]));
}

#[test]
fn scans_several_roots_and_gives_an_overlap_one_time() {
    let tree = Tree::new();
    tree.files(&["a/1", "a/b/2", "c/3", "d/4"]);
    let mut scope = tree.scope(&[]);
    scope.roots = vec!["~/a".into(), "~/a/b".into(), "~/c/".into()];

    let index = tree.scan(&scope);

    assert_eq!(
        tree.relative_paths(&index),
        set(&["a/1", "a/b/", "a/b/2", "c/3"])
    );
    assert_eq!(index.len(), 4);
}

#[test]
fn walks_a_root_that_an_ignore_entry_names() {
    let tree = Tree::new();
    tree.files(&["tmp/keep/x", "tmp/other/y"]);
    let mut scope = tree.scope(&INITIAL_IGNORE);
    scope.roots = vec!["~/tmp/keep".into()];

    let index = tree.scan(&scope);

    assert_eq!(tree.relative_paths(&index), set(&["tmp/keep/x"]));
}

/// Gives the permission back, so that the temporary tree can be removed.
struct Locked(PathBuf);

impl Drop for Locked {
    fn drop(&mut self) {
        fs::set_permissions(&self.0, Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn does_not_enter_an_ignored_directory() {
    let tree = Tree::new();
    let locked = Locked(tree.dir("web/node_modules/locked"));
    fs::set_permissions(&locked.0, Permissions::from_mode(0o000)).unwrap();

    let with_rule = tree.scan(&tree.scope(&["node_modules"]));
    let without_rule = tree.scan(&tree.scope(&[]));

    assert_eq!(with_rule.errors(), 0);
    assert_eq!(without_rule.errors(), 1);
}

#[test]
fn counts_a_missing_root_as_an_error() {
    let tree = Tree::new();
    let mut scope = tree.scope(&[]);
    scope.roots = vec!["~/missing".into()];

    let index = tree.scan(&scope);

    assert!(index.is_empty());
    assert_eq!(index.errors(), 1);
}

#[test]
fn stops_when_cancelled() {
    let tree = Tree::new();
    tree.files(&["a/1", "b/2"]);

    let result = scan(&tree.scope(&[]), 2, &AtomicBool::new(true));

    assert_eq!(result.err(), Some(ScanError::Cancelled));
}

#[test]
fn rejects_a_scope_with_a_bad_pattern() {
    let tree = Tree::new();

    let result = scan(&tree.scope(&["[abc"]), 2, &AtomicBool::new(false));

    assert!(matches!(
        result,
        Err(ScanError::Scope(ScopeError::BadPattern { .. }))
    ));
}

#[test]
fn an_empty_root_list_gives_an_empty_index() {
    let tree = Tree::new();
    let scope = Scope {
        roots: vec![],
        ..tree.scope(&[])
    };

    let index = tree.scan(&scope);

    assert!(index.is_empty());
    assert!(index.scanned_at().is_some());
}

/// R14 acceptance: `f palette-zoom` shows `palette-zoom.png`, and a file below
/// `~/tmp` or `~/go/pkg/mod` is not in the results.
#[test]
fn r14_finds_palette_zoom_and_nothing_below_tmp_or_the_go_module_cache() {
    let tree = Tree::new();
    tree.files(&[
        "lolcat-evidence/kukka/font-size-keys-palette/screenshots-final/palette-zoom.png",
        "tmp/kukka/palette-zoom.png",
        "go/pkg/mod/github.com/x/palette-zoom.png",
        "tmp/go/pkg/mod/palette-zoom.png",
    ]);
    let index = tree.scan(&tree.scope(&INITIAL_IGNORE));

    let hits = index.search("palette-zoom", 10);

    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits[0].path,
        tree.path(
            "lolcat-evidence/kukka/font-size-keys-palette/screenshots-final/palette-zoom.png"
        )
    );
}
