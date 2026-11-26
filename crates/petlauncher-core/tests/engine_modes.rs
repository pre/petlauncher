//! The mode decision of the engine, the order of the general search, R3 and
//! the cancel of a search.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use common::{echo_row, fake_applications, search, search_with, titles, TempHome};
use petlauncher_core::config::{Settings, Shortcut};
use petlauncher_core::{
    Action, CancelToken, Engine, Mode, Outcome, Provider, Results, Row, RowKind, Slot,
    SystemCommand,
};

fn echo(kind: RowKind, name: &'static str) -> impl Provider {
    move |text: &str, _: &Settings, _: &CancelToken| vec![echo_row(kind, name, text)]
}

/// An engine with one echo provider in each slot except the system commands.
fn engine_with_all_slots(home: &TempHome) -> Engine {
    let mut engine = home.engine();
    engine
        .register(Slot::SettingsPages, echo(RowKind::SettingsPage, "page"))
        .register(Slot::Dictionary, echo(RowKind::Dictionary, "dict"))
        .register(Slot::FileSearch, echo(RowKind::File, "file"))
        .register(Slot::Applications, echo(RowKind::Application, "app"))
        .register(Slot::Time, echo(RowKind::Time, "time"))
        .register(Slot::Calculator, echo(RowKind::Calculation, "calc"));
    engine
}

#[test]
fn an_empty_query_gives_no_rows() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);

    let results = search(&engine, " \n");

    assert_eq!(results.mode, Mode::Empty);
    assert!(results.rows.is_empty());
    assert_eq!(results.selected, None);
}

#[test]
fn the_general_search_shows_calculation_time_applications_system_and_settings_in_this_order() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);

    let results = search(&engine, "lock");

    assert_eq!(results.mode, Mode::General);
    assert_eq!(
        titles(&results),
        [
            "calc: lock",
            "time: lock",
            "app: lock",
            "Lock",
            "page: lock"
        ]
    );
    assert_eq!(results.selected, Some(0));
    assert_eq!(results.rows[3].enter, Action::System(SystemCommand::Lock));
}

/// An application provider that gives fixed rows for each text, as a
/// search that also matches inside words and in keywords does.
fn applications(names: &'static [&'static str]) -> impl Provider {
    move |_: &str, _: &Settings, _: &CancelToken| {
        names
            .iter()
            .map(|name| Row::new(RowKind::Application, *name, Action::Nothing))
            .collect()
    }
}

#[test]
fn a_system_command_with_the_query_in_its_title_comes_before_an_application_without_it() {
    let home = TempHome::new();
    let mut engine = home.engine();
    engine.register(Slot::Applications, applications(&["Clocks"]));

    let results = search(&engine, "lock");

    assert_eq!(titles(&results), ["Lock", "Clocks"]);
}

#[test]
fn the_rows_with_the_query_in_the_title_keep_the_order_of_the_slots() {
    let home = TempHome::new();
    let mut engine = home.engine();
    engine
        .register(Slot::Applications, applications(&["Slack", "Mouse"]))
        .register(Slot::SettingsPages, echo(RowKind::SettingsPage, "Sound"));

    let results = search(&engine, "s");

    assert_eq!(
        titles(&results),
        [
            "Slack",
            "Suspend",
            "Sound: s",
            "Mouse",
            "Lock",
            "Log out",
            "Power off"
        ]
    );
}

#[test]
fn the_calculation_and_the_time_stay_first_without_the_query_in_their_titles() {
    let home = TempHome::new();
    let mut engine = home.engine();
    engine
        .register(Slot::Applications, applications(&["Pinta"]))
        .register(
            Slot::Calculator,
            |_: &str, _: &Settings, _: &CancelToken| {
                vec![Row::new(
                    RowKind::Calculation,
                    "3.14159265358979",
                    Action::Nothing,
                )]
            },
        );

    let results = search(&engine, "pi");

    assert_eq!(titles(&results), ["3.14159265358979", "Pinta"]);
}

#[test]
fn the_general_search_gives_the_query_without_spaces_at_its_ends() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);

    let results = search(&engine, "  two words ");

    assert_eq!(results.rows[0].title, "calc: two words");
    assert_eq!(results.query, "  two words ");
}

#[test]
fn r3_include_applications_off_removes_the_applications_from_the_search() {
    let home = TempHome::new();
    let mut engine = home.engine();
    engine.register(
        Slot::Applications,
        fake_applications(home.dir("applications")),
    );
    let settings = Settings {
        include_applications: false,
        ..Settings::default()
    };

    let off = search_with(&engine, "firefox", &settings, &[]);
    let on = search_with(&engine, "firefox", &Settings::default(), &[]);

    assert!(off.rows.is_empty());
    assert_eq!(titles(&on), ["Firefox"]);
}

#[test]
fn r14_the_file_keyword_and_a_space_give_only_the_file_search_with_the_argument() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);

    let results = search(&engine, "f  palette-zoom ");

    assert_eq!(results.mode, Mode::FileSearch);
    assert_eq!(titles(&results), ["file: palette-zoom"]);
}

#[test]
fn r14_text_with_no_keyword_does_no_file_search() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);

    let results = search(&engine, "palette-zoom");

    assert!(!titles(&results).contains(&"file: palette-zoom"));
}

#[test]
fn r14_the_file_keyword_alone_is_a_general_search() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);

    let results = search(&engine, "f");

    assert_eq!(results.mode, Mode::General);
    assert_eq!(results.rows[0].title, "calc: f");
}

#[test]
fn r19_the_dictionary_keyword_and_a_space_give_only_the_dictionary() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);

    let results = search(&engine, "d launch");

    assert_eq!(results.mode, Mode::Dictionary);
    assert_eq!(titles(&results), ["dict: launch"]);
}

#[test]
fn the_keywords_come_from_the_settings() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);
    let mut settings = Settings::default();
    settings.file_search.keyword = "find".into();
    settings.dictionary.keyword = "def".into();

    let file = search_with(&engine, "find x", &settings, &[]);
    let dict = search_with(&engine, "def y", &settings, &[]);
    let old = search_with(&engine, "f x", &settings, &[]);

    assert_eq!(titles(&file), ["file: x"]);
    assert_eq!(titles(&dict), ["dict: y"]);
    assert_eq!(old.mode, Mode::General);
}

#[test]
fn the_dictionary_keyword_wins_over_a_shortcut_with_the_same_keyword() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);
    let cambridge = Shortcut {
        keyword: "d".into(),
        cmd: "https://dictionary.cambridge.org/dictionary/english/%s".into(),
        ..Shortcut::default()
    };

    let results = search_with(&engine, "d word", &Settings::default(), &[cambridge]);

    assert_eq!(results.mode, Mode::Dictionary);
}

#[test]
fn a_shortcut_keyword_and_a_space_give_only_the_shortcut_rows() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);
    let jira = Shortcut {
        name: "Jira".into(),
        keyword: "jira".into(),
        cmd: "https://jira.example.com/browse/%s".into(),
        ..Shortcut::default()
    };

    let results = search_with(&engine, "jira ABC-1", &Settings::default(), &[jira]);

    assert_eq!(results.mode, Mode::Shortcut);
    assert_eq!(titles(&results), ["Jira"]);
}

#[test]
fn a_first_word_that_is_no_keyword_gives_the_general_search_of_the_full_text() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);

    let results = search(&engine, "foo bar");

    assert_eq!(results.mode, Mode::General);
    assert_eq!(results.rows[0].title, "calc: foo bar");
}

#[test]
fn a_path_query_does_not_ask_the_providers() {
    let home = TempHome::new();
    let engine = engine_with_all_slots(&home);

    let results = search(&engine, "~/no-such-thing");

    assert_eq!(results.mode, Mode::Path);
    assert!(results.rows.is_empty());
    assert_eq!(results.selected, None);
}

#[test]
fn the_providers_of_one_slot_come_in_the_order_of_registration() {
    let home = TempHome::new();
    let mut engine = home.engine();
    engine
        .register(Slot::Calculator, echo(RowKind::Calculation, "first"))
        .register(Slot::Calculator, echo(RowKind::Calculation, "second"));

    let results = search(&engine, "zz");

    assert_eq!(titles(&results), ["first: zz", "second: zz"]);
}

#[test]
fn a_cancelled_search_does_not_ask_the_next_providers() {
    let home = TempHome::new();
    let mut engine = home.engine();
    let later_calls = Arc::new(AtomicUsize::new(0));
    let counter = later_calls.clone();
    engine
        .register(
            Slot::Calculator,
            |text: &str, _: &Settings, cancel: &CancelToken| {
                cancel.cancel();
                vec![echo_row(RowKind::Calculation, "calc", text)]
            },
        )
        .register(
            Slot::Applications,
            move |_: &str, _: &Settings, _: &CancelToken| {
                counter.fetch_add(1, Ordering::Relaxed);
                Vec::<Row>::new()
            },
        );
    let shortcuts = [Shortcut {
        keyword: "g".into(),
        cmd: "https://example.com/%s".into(),
        is_default_search: true,
        ..Shortcut::default()
    }];

    let results = engine.search("zz", &Settings::default(), &shortcuts, &CancelToken::new());

    assert_eq!(later_calls.load(Ordering::Relaxed), 0);
    assert_eq!(titles(&results), ["calc: zz"]);
}

#[test]
fn a_cancelled_path_search_reads_no_directory_entries() {
    let home = TempHome::new();
    home.file("a.txt", 1);
    let engine = home.engine();
    let cancel = CancelToken::new();
    cancel.cancel();

    let results = engine.search("~/", &Settings::default(), &[], &cancel);

    assert!(results.rows.is_empty());
}

#[test]
fn the_engine_and_the_contract_types_can_go_to_another_thread() {
    fn send_and_sync<T: Send + Sync>() {}
    fn send_and_clone<T: Send + Clone + 'static>() {}

    send_and_sync::<Engine>();
    send_and_clone::<Results>();
    send_and_clone::<Row>();
    send_and_clone::<Action>();
    send_and_clone::<Outcome>();
    send_and_clone::<CancelToken>();
}

#[test]
fn a_search_on_a_worker_thread_gives_the_results_back() {
    let home = TempHome::new();
    let engine = Arc::new(engine_with_all_slots(&home));
    let worker_engine = engine.clone();

    let results: Results = std::thread::spawn(move || search(&worker_engine, "zz"))
        .join()
        .expect("the worker thread ends");

    assert_eq!(results.query, "zz");
    assert_eq!(results.rows[0].title, "calc: zz");
}
