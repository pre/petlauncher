//! The stored index: save, load, and the rejection of a file that does not
//! fit.

mod common;

use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use common::Tree;
use petlauncher_index::store::{self, LoadError, FILE_NAME, FORMAT_VERSION};
use petlauncher_index::{FileEntry, Index, Scope};

fn entry(path: &[u8], is_dir: bool, modified: u64) -> FileEntry {
    FileEntry {
        path: PathBuf::from(OsStr::from_bytes(path)),
        is_dir,
        modified: UNIX_EPOCH + Duration::from_secs(modified),
    }
}

fn sample_index() -> Index {
    Index::from_entries(
        vec![
            entry(b"/h/docs", true, 10),
            entry(b"/h/docs/Palette-Zoom.png", false, 20),
            entry(b"/h/docs/\xff-not-utf8", false, 30),
            entry(b"/h/\xc3\x84rger.txt", false, 40),
        ],
        UNIX_EPOCH + Duration::from_nanos(1_764_000_000_123_456_789),
    )
}

fn scope(tree: &Tree) -> Scope {
    tree.scope(&["~/tmp", "node_modules", "*-wt"])
}

#[test]
fn loads_the_index_that_it_saved() {
    let tree = Tree::new();
    let file = tree.path("cache/petlauncher").join(FILE_NAME);
    let index = sample_index();

    store::save(&file, &scope(&tree), &index).unwrap();
    let loaded = store::load(&file, &scope(&tree)).unwrap();

    assert_eq!(
        loaded.iter().collect::<Vec<_>>(),
        index.iter().collect::<Vec<_>>()
    );
    assert_eq!(loaded.scanned_at(), index.scanned_at());
    assert_eq!(loaded.errors(), index.errors());
    assert_eq!(
        loaded.search("palette-zoom", 5),
        index.search("palette-zoom", 5)
    );
    assert_eq!(loaded.search("ärger", 5).len(), 1);
}

#[test]
fn writes_one_file_and_leaves_no_temporary_file() {
    let tree = Tree::new();
    let file = tree.path("cache").join(FILE_NAME);
    store::save(&file, &scope(&tree), &Index::default()).unwrap();

    store::save(&file, &scope(&tree), &sample_index()).unwrap();

    let names: Vec<_> = fs::read_dir(tree.path("cache"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, [FILE_NAME]);
    assert_eq!(store::load(&file, &scope(&tree)).unwrap().len(), 4);
}

#[test]
fn reports_a_missing_file() {
    let tree = Tree::new();

    let result = store::load(&tree.path("none"), &scope(&tree));

    assert!(matches!(result, Err(LoadError::Missing)));
}

#[test]
fn rejects_a_file_with_an_other_format_version() {
    let tree = Tree::new();
    let file = tree.path(FILE_NAME);
    store::save(&file, &scope(&tree), &sample_index()).unwrap();
    let mut bytes = fs::read(&file).unwrap();
    bytes[8..12].copy_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
    fs::write(&file, bytes).unwrap();

    let result = store::load(&file, &scope(&tree));

    assert!(matches!(result, Err(LoadError::Version(v)) if v == FORMAT_VERSION + 1));
}

#[test]
fn rejects_a_file_of_other_roots_ignore_entries_or_home() {
    let tree = Tree::new();
    let file = tree.path(FILE_NAME);
    let saved = scope(&tree);
    store::save(&file, &saved, &sample_index()).unwrap();
    let others = [
        Scope {
            roots: vec!["~".into(), "/data".into()],
            ..saved.clone()
        },
        Scope {
            ignore: vec!["~/tmp".into(), "node_modules".into()],
            ..saved.clone()
        },
        Scope {
            home: PathBuf::from("/home/other"),
            ..saved.clone()
        },
        Scope {
            outside_ignore_files: true,
            ..saved.clone()
        },
    ];

    for other in others {
        let result = store::load(&file, &other);

        assert!(matches!(result, Err(LoadError::OtherScope)), "{other:?}");
    }
}

#[test]
fn rejects_a_damaged_file() {
    let tree = Tree::new();
    let file = tree.path(FILE_NAME);
    store::save(&file, &scope(&tree), &sample_index()).unwrap();
    let good = fs::read(&file).unwrap();
    let mut flipped = good.clone();
    let last = flipped.len() - 1;
    flipped[last] ^= 1;
    let mut other_magic = good.clone();
    other_magic[0] = b'X';
    let damaged = [
        flipped,
        good[..good.len() - 7].to_vec(),
        other_magic,
        good[..10].to_vec(),
        Vec::new(),
    ];

    for bytes in damaged {
        fs::write(&file, &bytes).unwrap();

        let result = store::load(&file, &scope(&tree));

        assert!(
            matches!(result, Err(LoadError::Damaged)),
            "{} bytes",
            bytes.len()
        );
    }
}

#[test]
fn gives_a_message_for_each_load_error() {
    let errors = [
        LoadError::Missing,
        LoadError::Io(std::io::Error::other("disk")),
        LoadError::Version(9),
        LoadError::OtherScope,
        LoadError::Damaged,
    ];

    for error in errors {
        assert!(!error.to_string().is_empty());
    }
}

#[test]
fn reports_an_error_when_the_cache_directory_cannot_be_made() {
    let tree = Tree::new();
    let blocker = tree.file("cache");

    let result = store::save(&blocker.join(FILE_NAME), &scope(&tree), &sample_index());

    assert!(result.is_err());
}
