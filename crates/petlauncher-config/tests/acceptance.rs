//! The acceptance steps of the requirements that need no user interface.

use std::fs;

use petlauncher_config::{
    load_settings, Browser, ConfigFile, ConfigStore, KeepQuery, Reload, Settings, Shortcut,
};

const OWNER_SETTINGS: &str = r#"# The owner's settings, with comments.

window_width = 750   # my width

[file_search]
keyword = "f"
# Large directories that I never search.
ignore = [
    "node_modules",
    "~/tmp",        # TMPDIR
]
"#;

/// R21, step 4: after an ignore path is added in the settings window, the
/// file has the new path and the comments are not changed.
#[test]
fn r21_an_added_ignore_path_is_in_the_file_and_the_comments_stay() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let path = ConfigFile::Settings.path_in(dir.path());
    fs::write(&path, OWNER_SETTINGS).expect("write settings");
    let mut store = ConfigStore::open(dir.path());

    store
        .edit_settings(|editor| {
            editor.add_ignore("~/lolcat-evidence");
        })
        .expect("edit");

    let text = fs::read_to_string(&path).expect("read");
    let mut new_lines = text.lines();
    for old_line in OWNER_SETTINGS.lines() {
        assert!(
            new_lines.any(|line| line == old_line),
            "line {old_line:?} is not in:\n{text}"
        );
    }
    assert!(text.contains("    \"~/lolcat-evidence\",\n"), "{text}");
    assert!(store
        .settings()
        .file_search
        .ignore
        .contains(&"~/lolcat-evidence".to_owned()));
}

/// R3, R5, R9, R14: the initial values with no settings file.
#[test]
fn the_initial_settings_are_the_values_of_the_requirements() {
    let dir = tempfile::tempdir().expect("temporary directory");

    let settings = load_settings(dir.path()).expect("initial settings");

    assert!(settings.close_on_focus_loss);
    assert!(settings.run_in_background);
    assert!(settings.show_tray_icon);
    assert!(settings.include_applications);
    assert_eq!(settings.window_width, 750);
    assert_eq!(settings.keep_query, KeepQuery::Seconds(30));
    assert_eq!(settings.file_search.keyword, "f");
    assert_eq!(settings.dictionary.keyword, "d");
    assert_eq!(settings, Settings::default());
}

/// R7 and R12: a shortcut with a target for no argument and with a
/// browser keeps the two fields through a write and a read.
#[test]
fn r7_r12_a_shortcut_keeps_its_target_without_argument_and_its_browser() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut store = ConfigStore::open(dir.path());
    let pull_requests = Shortcut {
        name: "Pull requests".to_owned(),
        keyword: "pr".to_owned(),
        cmd: "https://code.example.com/project/pull/%s".to_owned(),
        cmd_without_argument: "https://code.example.com/project/pulls".to_owned(),
        browser: Some(Browser::App("example-browser.desktop".to_owned())),
        ..Shortcut::default()
    };

    store
        .edit_shortcuts(|editor| {
            editor.add(&pull_requests);
            Ok(())
        })
        .expect("edit");
    let mut again = ConfigStore::open(dir.path());

    assert_eq!(again.shortcuts(), &[pull_requests]);
    assert_eq!(again.reload(ConfigFile::Shortcuts), Reload::Unchanged);
}

/// R9: the settings window can set each form of the time.
#[test]
fn r9_each_form_of_the_keep_query_time_goes_through_the_file() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut store = ConfigStore::open(dir.path());

    for value in [
        KeepQuery::Forever,
        KeepQuery::Disabled,
        KeepQuery::Seconds(90),
        KeepQuery::Seconds(600),
    ] {
        store
            .edit_settings(|editor| editor.set_keep_query(value))
            .expect("edit");

        assert_eq!(
            load_settings(dir.path()).expect("settings").keep_query,
            value
        );
    }
}
