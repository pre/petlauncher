//! The examples in `docs/settings.md` are valid files with no problems.

use std::fs;
use std::path::Path;

use petlauncher_config::{ConfigFile, ConfigStore, Settings};

fn toml_examples() -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/settings.md");
    let text = fs::read_to_string(path).expect("read docs/settings.md");
    text.split("```toml\n")
        .skip(1)
        .map(|block| block.split("```").next().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn the_examples_in_the_settings_document_are_valid() {
    let examples = toml_examples();
    assert_eq!(examples.len(), 2);
    let dir = tempfile::tempdir().expect("temporary directory");
    fs::write(ConfigFile::Settings.path_in(dir.path()), &examples[0]).expect("write");
    fs::write(ConfigFile::Shortcuts.path_in(dir.path()), &examples[1]).expect("write");

    let store = ConfigStore::open(dir.path());

    assert_eq!(store.error(ConfigFile::Settings), None);
    assert_eq!(store.error(ConfigFile::Shortcuts), None);
    assert_eq!(store.warnings(), vec![]);
    assert_eq!(store.settings().window_width, 900);
    assert_ne!(store.settings(), &Settings::default());
    assert_eq!(store.shortcuts().len(), 4);
    assert!(store.shortcuts()[3].cmd.starts_with("#!/bin/sh\n"));
}
