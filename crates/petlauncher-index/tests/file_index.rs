//! The API for the program: stored index at the start, background rescan,
//! change of the scope (R21), and the search from other threads.

mod common;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use common::{Tree, INITIAL_IGNORE};
use petlauncher_index::store::{self, LoadError, FILE_NAME};
use petlauncher_index::{FileIndex, Scope, ScopeError};

const PALETTE_ZOOM: &str =
    "daybook-evidence/kukka/font-size-keys-palette/screenshots-final/palette-zoom.png";

fn cache_dir(tree: &Tree) -> PathBuf {
    tree.path(".cache/petlauncher")
}

fn open(tree: &Tree, scope: Scope) -> FileIndex {
    FileIndex::open(scope, &cache_dir(tree)).expect("open the file index")
}

fn scanned(tree: &Tree, scope: Scope) -> FileIndex {
    let index = open(tree, scope);
    assert!(index.rescan());
    index.wait();
    index
}

#[test]
fn open_answers_from_the_stored_index_before_a_scan() {
    let tree = Tree::new();
    tree.file("notes/plan.md");
    drop(scanned(&tree, tree.scope(&[])));
    fs::remove_file(tree.path("notes/plan.md")).unwrap();

    let index = open(&tree, tree.scope(&[]));

    assert!(!index.is_scanning());
    assert_eq!(index.search("plan", 5)[0].path, tree.path("notes/plan.md"));
}

#[test]
fn open_ignores_a_stored_index_of_other_ignore_entries() {
    let tree = Tree::new();
    tree.file("notes/plan.md");
    drop(scanned(&tree, tree.scope(&[])));

    let index = open(&tree, tree.scope(&["~/tmp"]));

    assert!(index.snapshot().is_empty());
    assert!(index.is_older_than(Duration::from_secs(3600)));
}

#[test]
fn rescan_replaces_the_snapshot_and_writes_the_file() {
    let tree = Tree::new();
    let index = open(&tree, tree.scope(&[]));
    tree.file("notes/plan.md");
    let before = index.snapshot();

    assert!(index.rescan());
    index.wait();

    assert!(before.is_empty());
    assert_eq!(index.search("plan", 5).len(), 1);
    let stored = store::load(&cache_dir(&tree).join(FILE_NAME), &tree.scope(&[])).unwrap();
    assert_eq!(stored.len(), index.snapshot().len());
    assert_eq!(index.last_error(), None);
}

#[test]
fn is_older_than_is_true_before_a_scan_and_false_just_after_it() {
    let tree = Tree::new();
    let index = open(&tree, tree.scope(&[]));
    let hour = Duration::from_secs(3600);

    let before = index.is_older_than(hour);
    index.rescan();
    index.wait();

    assert!(before);
    assert!(!index.is_older_than(hour));
    assert!(index.is_older_than(Duration::ZERO));
}

/// R21 acceptance, steps 1 to 3, as far as the crate can do them.
#[test]
fn r21_an_added_ignore_path_removes_the_file_from_the_results_immediately() {
    let tree = Tree::new();
    tree.file(PALETTE_ZOOM);
    let index = scanned(&tree, tree.scope(&INITIAL_IGNORE));
    let found_before = index.search("palette-zoom", 10);
    let mut ignore = INITIAL_IGNORE.to_vec();
    ignore.push("~/daybook-evidence");

    let changed = index.set_scope(tree.scope(&ignore));
    index.wait();

    assert_eq!(found_before.len(), 1);
    assert_eq!(found_before[0].path, tree.path(PALETTE_ZOOM));
    assert_eq!(changed, Ok(true));
    assert!(index.search("palette-zoom", 10).is_empty());
    let file = cache_dir(&tree).join(FILE_NAME);
    assert!(store::load(&file, &tree.scope(&ignore)).is_ok());
    assert!(matches!(
        store::load(&file, &tree.scope(&INITIAL_IGNORE)),
        Err(LoadError::OtherScope)
    ));
}

#[test]
fn set_scope_with_an_equal_scope_does_not_scan() {
    let tree = Tree::new();
    let index = open(&tree, tree.scope(&INITIAL_IGNORE));

    let changed = index.set_scope(tree.scope(&INITIAL_IGNORE));

    assert_eq!(changed, Ok(false));
    assert!(!index.is_scanning());
    assert!(index.snapshot().scanned_at().is_none());
}

#[test]
fn set_scope_rejects_a_bad_pattern_and_keeps_the_old_scope() {
    let tree = Tree::new();
    tree.file("notes/plan.md");
    let index = open(&tree, tree.scope(&[]));

    let changed = index.set_scope(tree.scope(&["[notes"]));
    index.rescan();
    index.wait();

    assert!(matches!(changed, Err(ScopeError::BadPattern { .. })));
    assert_eq!(index.search("plan", 5).len(), 1);
}

#[test]
fn open_rejects_an_empty_ignore_entry() {
    let tree = Tree::new();

    let result = FileIndex::open(tree.scope(&[""]), &cache_dir(&tree));

    assert!(matches!(result, Err(ScopeError::EmptyIgnoreEntry)));
}

#[test]
fn searches_from_other_threads_while_rescans_run() {
    let tree = Tree::new();
    tree.files(&["a/palette-zoom.png", "b/other.txt"]);
    let index = Arc::new(scanned(&tree, tree.scope(&[])));
    let expected = index.search("palette-zoom", 5);

    let searchers: Vec<_> = (0..4)
        .map(|_| {
            let index = Arc::clone(&index);
            thread::spawn(move || {
                (0..300)
                    .map(|_| index.search("palette-zoom", 5))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    for _ in 0..20 {
        index.rescan();
        index.wait();
    }

    assert_eq!(expected.len(), 1);
    for searcher in searchers {
        for hits in searcher.join().unwrap() {
            assert_eq!(hits, expected);
        }
    }
}

#[test]
fn reports_a_write_error_and_still_uses_the_new_snapshot() {
    let tree = Tree::new();
    tree.file("notes/plan.md");
    let blocker = tree.file(".cache/blocker");
    let index = FileIndex::open(tree.scope(&[]), &blocker).unwrap();

    index.rescan();
    index.wait();

    assert!(index.last_error().is_some_and(|e| e.contains("blocker")));
    assert_eq!(index.search_files("plan", 5).len(), 1);
}
