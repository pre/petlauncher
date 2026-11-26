//! The command `petlauncher convert-shortcuts` with an invented Ulauncher
//! file (R6).

use std::fs;
use std::path::{Path, PathBuf};

use petlauncher_config::convert::{convert_shortcuts_file, ConvertError};
use petlauncher_config::{load_shortcuts, ConfigError, ConfigFile, Shortcut};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ulauncher-shortcuts.json")
}

/// A copy of the fixture, so that a test can prove that the converter does
/// not change the Ulauncher file.
fn ulauncher_copy(dir: &Path) -> PathBuf {
    let copy = dir.join("ulauncher-shortcuts.json");
    fs::copy(fixture(), &copy).expect("copy fixture");
    copy
}

#[test]
fn writes_shortcuts_toml_from_the_ulauncher_file() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let ulauncher = ulauncher_copy(dir.path());
    let config = dir.path().join("petlauncher");

    let summary = convert_shortcuts_file(&ulauncher, &config, false).expect("convert");

    let shortcuts = load_shortcuts(&config).expect("valid shortcuts.toml");
    let keywords: Vec<&str> = shortcuts.iter().map(|s| s.keyword.as_str()).collect();
    assert_eq!(keywords, ["s", "dd", "rv", "board"]);
    assert_eq!(
        shortcuts[3],
        Shortcut {
            name: "Local board".to_owned(),
            keyword: "board".to_owned(),
            cmd: "http://localhost:9999/#board".to_owned(),
            icon: "~/Pictures/board.png".to_owned(),
            run_without_argument: true,
            ..Shortcut::default()
        }
    );
    assert!(shortcuts[0].is_default_search);
    assert_eq!(summary.count, 4);
    assert_eq!(summary.path, ConfigFile::Shortcuts.path_in(&config));
    assert_eq!(summary.renamed.len(), 1);
    assert_eq!(summary.problems, vec![]);
}

#[test]
fn writes_the_script_as_a_readable_multi_line_string() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let ulauncher = ulauncher_copy(dir.path());

    convert_shortcuts_file(&ulauncher, dir.path(), false).expect("convert");

    let text = fs::read_to_string(ConfigFile::Shortcuts.path_in(dir.path())).expect("read");
    assert!(
        text.contains(
            "cmd = \"\"\"\n#!/bin/sh\nexec example-browser \"https://review.example.com/pull/$1\"\n\"\"\"\n"
        ),
        "{text}"
    );
    assert!(!text.contains("browser ="), "{text}");
    assert!(text.starts_with("# Shortcuts of petlauncher."), "{text}");
}

#[test]
fn does_not_change_the_ulauncher_file() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let ulauncher = ulauncher_copy(dir.path());
    let before = fs::read(&ulauncher).expect("read");

    convert_shortcuts_file(&ulauncher, dir.path(), false).expect("convert");

    assert_eq!(fs::read(&ulauncher).expect("read"), before);
}

#[test]
fn does_not_write_over_an_existing_file_without_the_force_flag() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let ulauncher = ulauncher_copy(dir.path());
    let target = ConfigFile::Shortcuts.path_in(dir.path());
    fs::write(&target, "# my shortcuts\n").expect("write");

    let result = convert_shortcuts_file(&ulauncher, dir.path(), false);

    assert_eq!(result, Err(ConvertError::TargetExists(target.clone())));
    assert_eq!(
        fs::read_to_string(&target).expect("read"),
        "# my shortcuts\n"
    );
}

#[test]
fn writes_over_an_existing_file_with_the_force_flag() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let ulauncher = ulauncher_copy(dir.path());
    let target = ConfigFile::Shortcuts.path_in(dir.path());
    fs::write(&target, "# my shortcuts\n").expect("write");

    let summary = convert_shortcuts_file(&ulauncher, dir.path(), true).expect("convert");

    assert_eq!(summary.count, 4);
    assert_eq!(load_shortcuts(dir.path()).expect("valid").len(), 4);
}

#[test]
fn uses_the_keyword_of_the_dictionary_in_the_settings() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let ulauncher = ulauncher_copy(dir.path());
    fs::write(
        ConfigFile::Settings.path_in(dir.path()),
        "[dictionary]\nkeyword = \"s\"\n",
    )
    .expect("write settings");

    convert_shortcuts_file(&ulauncher, dir.path(), false).expect("convert");

    let keywords: Vec<String> = load_shortcuts(dir.path())
        .expect("valid")
        .into_iter()
        .map(|shortcut| shortcut.keyword)
        .collect();
    assert_eq!(keywords, ["ss", "d", "rv", "board"]);
}

#[test]
fn stops_when_the_settings_file_has_an_error() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let ulauncher = ulauncher_copy(dir.path());
    fs::write(
        ConfigFile::Settings.path_in(dir.path()),
        "window_width = \n",
    )
    .expect("write");

    let result = convert_shortcuts_file(&ulauncher, dir.path(), false);

    assert!(
        matches!(result, Err(ConvertError::Config(ConfigError::Toml { .. }))),
        "{result:?}"
    );
    assert!(!ConfigFile::Shortcuts.path_in(dir.path()).exists());
}

#[test]
fn gives_a_read_error_for_a_missing_ulauncher_file() {
    let dir = tempfile::tempdir().expect("temporary directory");

    let result = convert_shortcuts_file(&dir.path().join("missing.json"), dir.path(), false);

    assert!(
        matches!(result, Err(ConvertError::Read { .. })),
        "{result:?}"
    );
}
