//! The acceptance steps of the requirements, through `Launcher::search` and
//! `Launcher::activate`, on a temporary home directory.

mod common;

use std::fs;
use std::time::Duration;

use common::{search, settings_with_comments, titles, Fixture, PALETTE_ZOOM};
use petlauncher_config::KeepQuery;
use petlauncher_core::{Action, CloseReason, Key, Mode, RowKind};

// R2 step 1.
#[test]
fn r2_ctrl_enter_on_an_application_shows_its_desktop_file_and_does_not_start_it() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let results = search(&launcher, "firefox");
    let firefox = common::row_index(&results, "Firefox");

    let outcome = launcher.activate(&results, Some(firefox), Key::CtrlEnter);

    assert_eq!(
        outcome.action,
        Action::ShowInFolder(fixture.desktop_file("firefox.desktop"))
    );
    assert!(outcome.close);
    assert!(!outcome.keep_query);
}

// R2 step 2.
#[test]
fn r2_ctrl_enter_on_a_calculation_does_nothing() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let results = search(&launcher, "42+1");

    let outcome = launcher.activate(&results, results.selected, Key::CtrlEnter);

    assert_eq!(titles(&results)[0], "43");
    assert_eq!(outcome.action, Action::Nothing);
    assert!(!outcome.close);
}

#[test]
fn r13_shift_enter_on_an_application_copies_the_path_of_its_desktop_file() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let results = search(&launcher, "firefox");

    let outcome = launcher.activate(&results, results.selected, Key::ShiftEnter);

    let path = fixture.desktop_file("firefox.desktop");
    assert_eq!(outcome.action, Action::Copy(path.display().to_string()));
}

#[test]
fn enter_on_an_application_starts_its_desktop_entry() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let results = search(&launcher, "fire");

    let outcome = launcher.activate(&results, results.selected, Key::Enter);

    let row = &results.rows[0];
    assert_eq!(row.kind, RowKind::Application);
    assert_eq!(row.subtitle, "Browse the web");
    assert_eq!(
        outcome.action,
        Action::LaunchDesktopEntry(fixture.desktop_file("firefox.desktop"))
    );
}

// R7 steps 1 and 2.
#[test]
fn r7_a_shortcut_opens_target_a_with_an_argument_and_target_b_without_one() {
    let fixture = Fixture::new();
    fixture.write_shortcuts(
        "[[shortcut]]\n\
         name = \"Rektangel pull requests\"\n\
         keyword = \"rektangel\"\n\
         cmd = \"https://github.com/LOLm/cat/pull/%s\"\n\
         cmd_without_argument = \"https://github.com/LOLm/cat/pulls\"\n",
    );
    let launcher = fixture.start();

    let with_argument = search(&launcher, "rektangel 123");
    let without_argument = search(&launcher, "rektangel");

    let url = |url: &str| Action::OpenUrl {
        url: url.into(),
        browser: None,
    };
    assert_eq!(
        launcher
            .activate(&with_argument, with_argument.selected, Key::Enter)
            .action,
        url("https://github.com/LOLm/cat/pull/123")
    );
    assert_eq!(
        launcher
            .activate(&without_argument, without_argument.selected, Key::Enter)
            .action,
        url("https://github.com/LOLm/cat/pulls")
    );
}

// R8 steps 1 and 2.
#[test]
fn r8_a_sum_shows_the_exact_result_and_enter_copies_it_and_closes_the_window() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "42.3+68.7+14.1+0.5");
    let outcome = launcher.activate(&results, results.selected, Key::Enter);

    assert_eq!(results.rows[0].kind, RowKind::Calculation);
    assert_eq!(results.rows[0].title, "125.6");
    assert_eq!(outcome.action, Action::Copy("125.6".into()));
    assert!(outcome.close);
    assert!(outcome.keep_query);
}

#[test]
fn r15_a_currency_conversion_uses_the_stored_rates_and_shows_their_date() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "100 usd in eur");

    assert_eq!(results.rows[0].title, "95.13 EUR");
    assert_eq!(
        results.rows[0].subtitle,
        "ECB reference rates of 2026-11-25"
    );
    assert_eq!(results.rows[0].enter, Action::Copy("95.13".into()));
}

#[test]
fn r15_with_no_rates_the_currency_row_says_so_and_copies_nothing() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.dirs.cache.join(petlauncher_calc::CACHE_FILE)).unwrap();
    let launcher = fixture.start();

    let results = search(&launcher, "100 usd in eur");

    assert_eq!(results.rows[0].title, "No currency rates available");
    assert_eq!(results.rows[0].enter, Action::Nothing);
}

// R9 steps 1 to 7.
#[test]
fn r9_the_query_stays_for_30_seconds_after_a_loss_of_focus() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let sum = "42.3+68.7+14.1+0.5";

    // Steps 1 and 2.
    launcher.window_closed(sum, CloseReason::FocusLoss);
    fixture.clock.advance(Duration::from_secs(29));
    let reopened = launcher.window_opened();
    let rows = search(&launcher, reopened.as_deref().unwrap_or_default());

    // Step 3.
    launcher.window_closed(sum, CloseReason::FocusLoss);
    fixture.clock.advance(Duration::from_secs(31));
    let after_the_time = launcher.window_opened();

    assert_eq!(reopened.as_deref(), Some(sum));
    assert_eq!(rows.rows[0].title, "125.6");
    assert_eq!(after_the_time, None);
}

#[test]
fn r9_step_4_enter_on_a_calculation_keeps_the_query() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let results = search(&launcher, "42.3+68.7");
    let outcome = launcher.activate(&results, results.selected, Key::Enter);

    launcher.executed(&results.query, &outcome).unwrap();
    fixture.clock.advance(Duration::from_secs(5));

    assert_eq!(launcher.window_opened().as_deref(), Some("42.3+68.7"));
}

#[test]
fn r9_step_5_enter_on_an_application_clears_the_query() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let results = search(&launcher, "firefox");
    let outcome = launcher.activate(&results, results.selected, Key::Enter);

    launcher.executed(&results.query, &outcome).unwrap();

    assert_eq!(launcher.window_opened(), None);
}

#[test]
fn r9_step_6_disable_keeps_no_query() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    launcher
        .edit_settings(|editor| editor.set_keep_query(KeepQuery::Disabled))
        .unwrap();

    launcher.window_closed("42.3+68.7+14.1+0.5", CloseReason::FocusLoss);

    assert_eq!(launcher.window_opened(), None);
}

#[test]
fn r9_step_7_forever_keeps_the_query_after_30_seconds() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    launcher
        .edit_settings(|editor| editor.set_keep_query(KeepQuery::Forever))
        .unwrap();

    launcher.window_closed("42.3+68.7+14.1+0.5", CloseReason::FocusLoss);
    fixture.clock.advance(Duration::from_secs(3600));

    assert_eq!(
        launcher.window_opened().as_deref(),
        Some("42.3+68.7+14.1+0.5")
    );
}

// R14.
#[test]
fn r14_the_keyword_f_finds_the_file_and_no_file_below_tmp_or_go_pkg_mod() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let found = search(&launcher, "f palette-zoom");
    let no_keyword = search(&launcher, "palette-zoom");

    assert_eq!(found.mode, Mode::FileSearch);
    assert_eq!(titles(&found), ["palette-zoom.png"]);
    assert_eq!(found.rows[0].kind, RowKind::File);
    assert_eq!(found.rows[0].local_file, Some(fixture.home(PALETTE_ZOOM)));
    assert_eq!(no_keyword.mode, Mode::General);
    assert!(no_keyword
        .rows
        .iter()
        .all(|row| row.kind != RowKind::File && row.kind != RowKind::Directory));
}

#[test]
fn r14_the_keys_on_a_file_row_work_as_on_other_file_rows() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let results = search(&launcher, "f palette-zoom");
    let path = fixture.home(PALETTE_ZOOM);

    let enter = launcher.activate(&results, Some(0), Key::Enter);
    let ctrl = launcher.activate(&results, Some(0), Key::CtrlEnter);
    let shift = launcher.activate(&results, Some(0), Key::ShiftEnter);

    assert_eq!(enter.action, Action::OpenFile(path.clone()));
    assert_eq!(ctrl.action, Action::ShowInFolder(path.clone()));
    assert_eq!(shift.action, Action::Copy(path.display().to_string()));
    assert!(!enter.keep_query);
}

#[test]
fn r14_a_directory_row_changes_the_query_and_a_second_enter_opens_the_directory() {
    let fixture = Fixture::new();
    let launcher = fixture.start();
    let results = search(&launcher, "f L0L3-IMAGE");
    let directory = fixture.home("lolcat-output/L0L3-IMAGE");

    let enter = launcher.activate(&results, Some(0), Key::Enter);
    let ctrl = launcher.activate(&results, Some(0), Key::CtrlEnter);
    let Action::SetQuery(query) = &enter.action else {
        panic!("{:?}", enter.action);
    };
    let typed = search(&launcher, query);
    let second_enter = launcher.activate(&typed, typed.selected, Key::Enter);

    assert_eq!(results.rows[0].kind, RowKind::Directory);
    assert_eq!(query, "~/lolcat-output/L0L3-IMAGE/");
    assert!(!enter.close);
    assert_eq!(ctrl.action, Action::OpenFolder(directory.clone()));
    assert_eq!(second_enter.action, Action::OpenFolder(directory));
}

// R19 step 1, with the decision of the coordinator: the meanings of the
// typed word first, then the longer words.
#[test]
fn r19_d_launch_gives_the_meanings_of_launch_then_longer_words_that_start_with_launch() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "d launch");

    assert_eq!(results.mode, Mode::Dictionary);
    let (meanings, suggestions): (Vec<_>, Vec<_>) = results
        .rows
        .iter()
        .partition(|row| row.subtitle.starts_with("launch,"));
    assert_eq!(
        meanings.iter().map(|row| &row.subtitle).collect::<Vec<_>>(),
        [
            "launch, noun",
            "launch, verb",
            "launch, verb. Synonyms: start up"
        ]
    );
    let words: Vec<&str> = suggestions.iter().map(|row| row.title.as_str()).collect();
    assert!(words.contains(&"launcher"), "{words:?}");
    assert!(
        words.iter().all(|word| word.starts_with("launch")),
        "{words:?}"
    );
    assert!(!words.contains(&"launch"), "{words:?}");
    assert!(results.rows[..meanings.len()]
        .iter()
        .all(|row| row.subtitle.starts_with("launch,")));
}

// R19 steps 2 and 3.
#[test]
fn r19_d_launcher_gives_its_meanings_and_enter_opens_the_dictionary_window() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "d launcher");
    let outcome = launcher.activate(&results, results.selected, Key::Enter);

    assert_eq!(
        titles(&results),
        [
            "a device that fires rockets",
            "a machine that throws aircraft from a ship"
        ]
    );
    assert_eq!(
        results.rows[0].subtitle,
        "launcher, noun. Synonyms: rocket launcher"
    );
    assert_eq!(
        results.rows[1].subtitle,
        "launcher, noun. Synonyms: catapult"
    );
    assert_eq!(outcome.action, Action::OpenDictionary("launcher".into()));
    assert!(outcome.close);
    assert!(!outcome.keep_query);
}

#[test]
fn r19_a_missing_wordnet_directory_gives_one_row_with_the_install_command_and_the_path() {
    let fixture = Fixture::new();
    let missing = fixture.root.join("no-wordnet");
    fixture.write_settings(&format!(
        "[dictionary]\nwordnet_path = \"{}\"\n",
        missing.display()
    ));
    let launcher = fixture.start();

    let results = search(&launcher, "d launcher");

    assert_eq!(results.rows.len(), 1);
    assert!(results.rows[0]
        .title
        .contains("sudo apt install wordnet-base"));
    assert!(results.rows[0]
        .subtitle
        .contains(&missing.display().to_string()));
}

#[test]
fn r19_a_text_that_starts_no_word_gives_one_row_that_does_nothing() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "d xyzzy");

    assert_eq!(titles(&results), ["\"xyzzy\" is not in the dictionary"]);
    assert_eq!(results.rows[0].enter, Action::Nothing);
}

#[test]
fn the_dictionary_window_finds_words_through_the_same_dictionary() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let entries = launcher.dictionary_lookup("launcher").unwrap();
    let words = launcher.dictionary_suggestions("launch", 3).unwrap();

    assert_eq!(entries[0].word, "launcher");
    assert_eq!(entries[0].meanings.len(), 2);
    assert_eq!(words.len(), 3);
    assert_eq!(words[0], "launch");
}

// R20, the three forms of the requirement, at 18:04 on Thursday in Helsinki.
#[test]
fn r20_time_now_in_new_york_gives_the_time_in_new_york() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "time now in new york");

    assert_eq!(titles(&results), ["11:04, Thursday"]);
    assert_eq!(results.rows[0].kind, RowKind::Time);
    assert_eq!(
        results.rows[0].enter,
        Action::Copy("11:04, Thursday".into())
    );
}

#[test]
fn r20_a_time_in_las_vegas_gives_the_owner_time() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "17:00 in las vegas");

    assert_eq!(titles(&results), ["03:00, Friday"]);
    assert!(
        results.rows[0]
            .subtitle
            .starts_with("17:00 in Las Vegas, Nevada, United States (PST, UTC-8)"),
        "{}",
        results.rows[0].subtitle
    );
}

#[test]
fn r20_six_am_in_helsinki_gives_the_owner_time() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "6am in helsinki");
    let outcome = launcher.activate(&results, results.selected, Key::Enter);

    assert_eq!(titles(&results), ["06:00, Thursday"]);
    assert_eq!(outcome.action, Action::Copy("06:00, Thursday".into()));
    assert!(!outcome.keep_query);
}

// R21 steps 1 to 4.
#[test]
fn r21_an_added_ignore_path_removes_the_file_and_keeps_the_comments() {
    let fixture = Fixture::new();
    let original = settings_with_comments(&fixture.wordnet_dir());
    fixture.write_settings(&original);
    let launcher = fixture.start();

    // Step 1.
    let before = search(&launcher, "f palette-zoom");
    // Step 2.
    launcher
        .edit_settings(|editor| {
            editor.add_ignore("~/daybook-evidence");
        })
        .unwrap();
    // Step 3.
    launcher.wait_idle();
    let after = search(&launcher, "f palette-zoom");

    assert_eq!(titles(&before), ["palette-zoom.png"]);
    assert!(after.rows.is_empty(), "{:?}", titles(&after));
    // Step 4.
    let text = fs::read_to_string(fixture.dirs.config.join("settings.toml")).unwrap();
    assert!(text.contains("\"~/daybook-evidence\""), "{text}");
    for line in original.lines().filter(|line| line.contains('#')) {
        assert!(text.contains(line), "{line:?} is not in:\n{text}");
    }
    assert!(launcher
        .settings()
        .file_search
        .ignore
        .contains(&"~/daybook-evidence".to_owned()));
}

// R16: Enter on `lock` locks the screen. Clocks also matches `lock`, inside
// a word of its name, so it comes after the system command.
#[test]
fn r16_lock_gives_the_system_command_first_and_an_application_with_lock_inside_a_word_next() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "lock");
    let outcome = launcher.activate(&results, results.selected, Key::Enter);

    assert_eq!(titles(&results), ["Lock", "Clocks"]);
    assert_eq!(
        outcome.action,
        Action::System(petlauncher_core::SystemCommand::Lock)
    );
}

// R3 "Include applications in search" off.
#[test]
fn r3_with_applications_off_the_settings_pages_and_system_commands_stay() {
    let fixture = Fixture::new();
    fixture.write_settings(&format!(
        "include_applications = false\n[dictionary]\nwordnet_path = \"{}\"\n",
        fixture.wordnet_dir().display()
    ));
    let launcher = fixture.start();

    let fire = search(&launcher, "fire");
    let wifi = search(&launcher, "wifi");
    let lock = search(&launcher, "lock");

    assert!(fire.rows.iter().all(|row| row.kind != RowKind::Application));
    assert_eq!(titles(&wifi), ["Wi-Fi"]);
    assert_eq!(wifi.rows[0].kind, RowKind::SettingsPage);
    assert_eq!(titles(&lock), ["Lock"]);
}

#[test]
fn r16_a_settings_page_row_starts_its_desktop_entry_and_has_it_as_local_file() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "wireless");

    let path = fixture.desktop_file("gnome-wifi-panel.desktop");
    assert_eq!(titles(&results), ["Wi-Fi"]);
    assert_eq!(results.rows[0].subtitle, "Settings");
    assert_eq!(
        results.rows[0].enter,
        Action::LaunchDesktopEntry(path.clone())
    );
    assert_eq!(results.rows[0].local_file, Some(path));
}

#[test]
fn r1_a_typed_directory_still_opens_in_the_file_manager() {
    let fixture = Fixture::new();
    let launcher = fixture.start();

    let results = search(&launcher, "~/lolcat-output/L0L3-IMAGE/");
    let outcome = launcher.activate(&results, results.selected, Key::Enter);

    assert_eq!(results.mode, Mode::Path);
    assert_eq!(
        outcome.action,
        Action::OpenFolder(fixture.home("lolcat-output/L0L3-IMAGE"))
    );
}
