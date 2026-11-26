use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use petlauncher_dict::{Dictionary, Error};
use tempfile::TempDir;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wordnet")
}

/// Copies the fixture files to a new temporary directory, except `skip`.
fn copy_of_fixture(skip: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().expect("a temporary directory");
    for file in fs::read_dir(fixture()).expect("the fixture") {
        let file = file.expect("a fixture file");
        let name = file.file_name();
        if !skip.contains(&name.to_str().expect("a file name")) {
            fs::copy(file.path(), dir.path().join(&name)).expect("a copy");
        }
    }
    dir
}

#[test]
fn fixture_directory_opens_with_one_row_for_each_word() {
    let dictionary = Dictionary::open(fixture()).expect("a dictionary");

    let text = format!("{dictionary:?}");

    assert_eq!(text, "Dictionary { words: 51, .. }");
}

#[test]
fn dictionary_can_be_shared_with_a_different_thread() {
    let dictionary = std::sync::Arc::new(Dictionary::open(fixture()).expect("a dictionary"));
    let shared = std::sync::Arc::clone(&dictionary);

    let words = std::thread::spawn(move || shared.suggestions("launch", 1))
        .join()
        .expect("a thread");

    assert_eq!(words, ["launch"]);
    assert_eq!(dictionary.suggestions("launch", 1), words);
}

#[test]
fn missing_directory_says_that_wordnet_base_is_not_installed() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("wordnet");

    let error = Dictionary::open(&path).expect_err("an error");

    assert!(matches!(&error, Error::NotInstalled { path: p } if *p == path));
    let text = error.to_string();
    assert!(
        text.contains("the package wordnet-base is not installed"),
        "{text}"
    );
    assert!(text.contains(&path.display().to_string()), "{text}");
}

#[test]
fn directory_with_missing_files_names_the_files() {
    let dir = copy_of_fixture(&["data.noun", "verb.exc"]);

    let error = Dictionary::open(dir.path()).expect_err("an error");

    match &error {
        Error::Incomplete { path, missing } => {
            assert_eq!(path, dir.path());
            assert_eq!(missing, &["data.noun", "verb.exc"]);
        }
        other => panic!("not Incomplete: {other:?}"),
    }
    let text = error.to_string();
    assert!(
        text.contains("the package wordnet-base is not installed"),
        "{text}"
    );
    assert!(text.contains("data.noun, verb.exc"), "{text}");
    assert!(text.contains(&dir.path().display().to_string()), "{text}");
}

#[test]
fn bad_index_file_is_a_format_error() {
    let dir = copy_of_fixture(&[]);
    fs::write(dir.path().join("index.verb"), "launch v\n").expect("a write");

    let error = Dictionary::open(dir.path()).expect_err("an error");

    assert!(matches!(&error, Error::Format { path, .. } if path.ends_with("index.verb")));
    assert!(error.to_string().contains("line 1"), "{error}");
}

#[test]
fn unreadable_file_is_a_read_error() {
    let dir = copy_of_fixture(&[]);
    let file = dir.path().join("index.adv");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).expect("a mode change");
    if fs::read(&file).is_ok() {
        return; // The root user can read the file, so this test cannot fail it.
    }

    let error = Dictionary::open(dir.path()).expect_err("an error");

    assert!(matches!(&error, Error::Read { path, .. } if *path == file));
    assert!(std::error::Error::source(&error).is_some());
    assert!(error.to_string().contains("cannot read"), "{error}");
}

#[test]
fn data_file_that_does_not_match_the_index_is_a_format_error_at_lookup() {
    let dir = copy_of_fixture(&[]);
    fs::write(dir.path().join("data.noun"), "  1 a header only\n").expect("a write");
    let dictionary = Dictionary::open(dir.path()).expect("a dictionary");

    let error = dictionary.lookup("launcher").expect_err("an error");

    assert!(matches!(&error, Error::Format { path, .. } if path.ends_with("data.noun")));
    assert!(
        error.to_string().contains("is not a WordNet file"),
        "{error}"
    );
}

#[test]
fn missing_frequency_file_gives_suggestions_in_alphabetical_order() {
    let dir = copy_of_fixture(&["cntlist.rev"]);
    let dictionary = Dictionary::open(dir.path()).expect("a dictionary");

    let words = dictionary.suggestions("launch", 10);

    assert_eq!(
        words,
        [
            "launch",
            "launch pad",
            "launcher",
            "launching",
            "launching pad",
            "launchpad"
        ]
    );
}
