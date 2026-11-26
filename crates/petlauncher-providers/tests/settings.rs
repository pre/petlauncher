//! The settings through the `Launcher` (R3, R12, R18, R21): reload, errors,
//! what depends on the settings, the autostart file and the browsers.

mod common;

use std::fs;

use common::{search, settings_with_comments, titles, Fixture};
use petlauncher_config::{ConfigFile, Reload, Shortcut};
use petlauncher_core::{Mode, RowKind};
use petlauncher_providers::{Autostart, ScopeError, Update};

fn settings(fixture: &Fixture, extra: &str) -> String {
    format!(
        "{extra}\n[dictionary]\nwordnet_path = \"{}\"\n",
        fixture.wordnet_dir().display()
    )
}

#[test]
fn r18_a_broken_settings_file_keeps_the_old_settings_and_gives_the_error() {
    let fixture = Fixture::new();
    fixture.write_settings(&settings(&fixture, "window_width = 900"));
    let launcher = fixture.start();

    fixture.write_settings("window_width = \n");
    let result = launcher.reload(ConfigFile::Settings);

    assert_eq!(result, Reload::Failed);
    assert_eq!(launcher.settings().window_width, 900);
    let error = launcher
        .config_error(ConfigFile::Settings)
        .expect("an error");
    assert_eq!(error.line(), Some(1));
    assert_eq!(
        titles(&search(&launcher, "d launcher"))[0],
        "a device that fires rockets"
    );
}

#[test]
fn r18_the_watcher_reads_a_saved_file_and_reports_the_error() {
    let fixture = Fixture::new();
    fixture.write_settings(&settings(&fixture, "window_width = 900"));
    let mut hooks = fixture.hooks();
    hooks.watch_config = true;
    let launcher = fixture.start_with(hooks);

    fixture.write_settings("window_width = \"wide\"\n");
    let update = fixture.wait_for_update(|update| update == Update::Config(ConfigFile::Settings));

    assert_eq!(update, Some(Update::Config(ConfigFile::Settings)));
    assert_eq!(launcher.settings().window_width, 900);
    assert!(launcher.config_error(ConfigFile::Settings).is_some());
}

#[test]
fn r18_the_watcher_puts_a_correct_file_in_effect() {
    let fixture = Fixture::new();
    let mut hooks = fixture.hooks();
    hooks.watch_config = true;
    let launcher = fixture.start_with(hooks);

    fixture.write_settings(&settings(&fixture, "window_width = 1200"));
    let update = fixture.wait_for_update(|update| update == Update::Config(ConfigFile::Settings));

    assert!(update.is_some());
    assert_eq!(launcher.settings().window_width, 1200);
    assert_eq!(launcher.config_error(ConfigFile::Settings), None);
}

#[test]
fn a_reload_gives_the_new_keywords_and_the_new_wordnet_directory_to_the_search() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    fixture.write_settings(&format!(
        "[file_search]\nkeyword = \"ff\"\n[dictionary]\nkeyword = \"dict\"\nwordnet_path = \"{}\"\n",
        fixture.root.join("other-wordnet").display()
    ));

    let result = launcher.reload(ConfigFile::Settings);
    launcher.wait_idle();

    assert_eq!(result, Reload::Changed);
    assert_eq!(search(&launcher, "ff palette-zoom").mode, Mode::FileSearch);
    assert_eq!(search(&launcher, "f palette-zoom").mode, Mode::General);
    let dictionary = search(&launcher, "dict launcher");
    assert_eq!(dictionary.mode, Mode::Dictionary);
    assert!(dictionary.rows[0].subtitle.contains("other-wordnet"));
}

#[test]
fn r21_a_reload_with_a_new_ignore_entry_updates_the_index_immediately() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    fixture.write_settings(&settings(
        &fixture,
        "[file_search]\nignore = [\"~/tmp\", \"~/go/pkg/mod\", \"daybook-evidence\"]",
    ));

    launcher.reload(ConfigFile::Settings);
    launcher.wait_idle();

    assert!(search(&launcher, "f palette-zoom").rows.is_empty());
}

#[test]
fn a_bad_ignore_pattern_keeps_the_old_scope_and_gives_the_error() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    fixture.write_settings(&settings(&fixture, "[file_search]\nignore = [\"[\"]"));

    let result = launcher.reload(ConfigFile::Settings);
    launcher.wait_idle();

    assert_eq!(result, Reload::Changed);
    assert!(matches!(
        launcher.file_search_error(),
        Some(ScopeError::BadPattern { entry, .. }) if entry == "["
    ));
    assert_eq!(
        titles(&search(&launcher, "f palette-zoom")),
        ["palette-zoom.png"]
    );
    assert!(search(&launcher, "f palette-zoom-in-tmp").rows.is_empty());
}

#[test]
fn a_bad_ignore_pattern_at_the_start_gives_the_initial_scope_and_the_error() {
    let fixture = Fixture::new();
    fixture.write_settings(&settings(&fixture, "[file_search]\nignore = [\"[\"]"));

    let launcher = fixture.start();

    assert!(launcher.file_search_error().is_some());
    assert_eq!(
        titles(&search(&launcher, "f palette-zoom")),
        ["palette-zoom.png"]
    );
    assert!(search(&launcher, "f palette-zoom-in-tmp").rows.is_empty());
}

#[test]
fn a_changed_shortcut_is_in_the_next_search() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    launcher
        .edit_shortcuts(|editor| {
            editor.add(&Shortcut {
                name: "Pull requests".into(),
                keyword: "pr".into(),
                cmd: "https://example.com/pull/%s".into(),
                ..Shortcut::default()
            });
            Ok(())
        })
        .unwrap();

    assert_eq!(launcher.shortcuts().len(), 1);
    let results = search(&launcher, "pr 7");
    assert_eq!(results.mode, Mode::Shortcut);
    assert_eq!(results.rows[0].kind, RowKind::Shortcut);
}

#[test]
fn the_warnings_of_the_settings_are_available() {
    let fixture = Fixture::new();
    fixture.write_settings(&settings(&fixture, "windw_width = 900"));

    let launcher = fixture.start();

    assert_eq!(launcher.config_warnings().len(), 1);
}

#[test]
fn r3_run_in_background_writes_and_removes_the_autostart_file() {
    let fixture = Fixture::new();
    let autostart = Autostart {
        config_home: fixture.home(".config"),
        command: "/opt/petlauncher/petlauncher --hidden".into(),
    };
    let file = autostart.path();
    let mut hooks = fixture.hooks();
    hooks.autostart = Some(autostart);
    let launcher = fixture.start_with(hooks);

    let at_the_start = fs::read_to_string(&file).unwrap();
    launcher
        .edit_settings(|editor| editor.set_run_in_background(false))
        .unwrap();
    let exists_when_off = file.exists();
    launcher
        .edit_settings(|editor| editor.set_run_in_background(true))
        .unwrap();

    assert!(at_the_start.contains("Exec=/opt/petlauncher/petlauncher --hidden\n"));
    assert!(!exists_when_off);
    assert!(file.exists());
}

#[test]
fn an_autostart_file_that_cannot_be_written_is_a_problem() {
    let fixture = Fixture::new();
    let blocker = fixture.file("not-a-directory");
    let mut hooks = fixture.hooks();
    hooks.autostart = Some(Autostart {
        config_home: blocker,
        command: "petlauncher".into(),
    });

    let launcher = fixture.start_with(hooks);

    let problems = launcher.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("start at login"), "{problems:?}");
}

#[test]
fn r12_the_browsers_are_the_installed_web_browsers_in_name_order() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let browsers = launcher.browsers();

    let ids: Vec<&str> = browsers.iter().map(|entry| entry.id.as_str()).collect();
    assert_eq!(ids, ["firefox.desktop", "microsoft-edge.desktop"]);
}

#[test]
fn the_comments_stay_when_the_settings_window_changes_a_value() {
    let fixture = Fixture::new();
    let original = settings_with_comments(&fixture.wordnet_dir());
    fixture.write_settings(&original);
    let launcher = fixture.start();

    launcher
        .edit_settings(|editor| editor.set_window_width(1000))
        .unwrap();

    let text = fs::read_to_string(fixture.dirs.config.join("settings.toml")).unwrap();
    assert_eq!(
        text,
        original.replace("window_width = 900 ", "window_width = 1000 ")
    );
    assert_eq!(launcher.settings().window_width, 1000);
}
