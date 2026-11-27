//! The acceptance steps of the requirements that need no user interface.

use std::fs;

use petlauncher_config::{
    load_settings, Browser, ConfigFile, ConfigStore, CustomPosition, KeepQuery, ProblemKind,
    Reload, Settings, Shortcut, WindowPosition,
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

/// The position of the launcher: the fixed places, and a pinned place that
/// the settings window writes with the setting "custom".
#[test]
fn the_window_position_and_a_pinned_place_go_through_the_file() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let mut store = ConfigStore::open(dir.path());
    assert_eq!(store.settings().window_position, WindowPosition::Center);
    assert_eq!(store.settings().custom_position, None);

    let mut fixed = Vec::new();
    for (position, name) in [
        (WindowPosition::Top, "top"),
        (WindowPosition::UpperQuarter, "upper-quarter"),
        (WindowPosition::UpperThird, "upper-third"),
        (WindowPosition::Center, "center"),
    ] {
        store
            .edit_settings(|editor| editor.set_window_position(position))
            .expect("edit");
        let text = fs::read_to_string(ConfigFile::Settings.path_in(dir.path())).expect("read");
        let loaded = load_settings(dir.path()).expect("settings");
        fixed.push((
            loaded.window_position == position,
            text.contains(&format!("window_position = \"{name}\"\n")),
        ));
    }
    store
        .edit_settings(|editor| editor.pin_position(CustomPosition { x: 0.25, y: 0.125 }))
        .expect("edit");
    let pinned = load_settings(dir.path()).expect("settings");
    let text = fs::read_to_string(ConfigFile::Settings.path_in(dir.path())).expect("read");

    assert_eq!(fixed, [(true, true); 4]);
    assert_eq!(pinned.window_position, WindowPosition::Custom);
    assert_eq!(
        pinned.custom_position,
        Some(CustomPosition { x: 0.25, y: 0.125 })
    );
    assert!(text.contains("window_position = \"custom\"\n"), "{text}");
    assert!(
        text.contains("custom_position = { x = 0.25, y = 0.125 }\n"),
        "{text}"
    );
}

#[test]
fn a_custom_place_out_of_the_work_area_is_an_error_and_custom_with_no_place_a_warning() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let path = ConfigFile::Settings.path_in(dir.path());
    fs::write(&path, "custom_position = { x = 1.5, y = 0.2 }\n").expect("write");
    let store = ConfigStore::open(dir.path());
    let error = store
        .error(ConfigFile::Settings)
        .expect("an error")
        .to_string();
    fs::write(&path, "window_position = \"custom\"\n").expect("write");
    let store = ConfigStore::open(dir.path());
    let warnings: Vec<ProblemKind> = store
        .warnings()
        .into_iter()
        .map(|problem| problem.kind)
        .collect();
    fs::write(&path, "window_position = \"middle\"\n").expect("write");
    let store = ConfigStore::open(dir.path());
    let wrong_name = store
        .error(ConfigFile::Settings)
        .expect("an error")
        .to_string();

    assert!(error.contains("custom_position"), "{error}");
    assert!(error.contains(":1:"), "{error}");
    assert_eq!(warnings, [ProblemKind::NoCustomPosition]);
    assert!(wrong_name.contains("middle"), "{wrong_name}");
}
