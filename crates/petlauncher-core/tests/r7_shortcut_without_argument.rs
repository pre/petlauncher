//! R6, R7 and R12: shortcut rows in the engine.

mod common;

use common::{fake_applications, fake_calculator, search_with, titles, TempHome};
use petlauncher_core::config::{Browser, Settings, Shortcut};
use petlauncher_core::{activate, Action, Key, Mode, Slot};

fn rektangel() -> Shortcut {
    Shortcut {
        name: "Rektangel pull request".into(),
        keyword: "rektangel".into(),
        cmd: "https://github.com/pre/rektangel/pull/%s".into(),
        cmd_without_argument: "https://github.com/pre/rektangel/pulls".into(),
        ..Shortcut::default()
    }
}

fn google() -> Shortcut {
    Shortcut {
        name: "Google".into(),
        keyword: "g".into(),
        cmd: "https://www.google.com/search?q=%s".into(),
        is_default_search: true,
        ..Shortcut::default()
    }
}

fn url(url: &str) -> Action {
    Action::OpenUrl {
        url: url.into(),
        browser: None,
    }
}

#[test]
fn r7_step1_rektangel_with_a_number_opens_that_pull_request() {
    let home = TempHome::new();
    let engine = home.engine();
    let results = search_with(
        &engine,
        "rektangel 123",
        &Settings::default(),
        &[rektangel()],
    );

    let outcome = activate(&results, results.selected, Key::Enter);

    assert_eq!(results.mode, Mode::Shortcut);
    assert_eq!(
        outcome.action,
        url("https://github.com/pre/rektangel/pull/123")
    );
    assert!(outcome.close);
    assert!(!outcome.keep_query);
}

#[test]
fn r7_step2_rektangel_with_no_argument_opens_the_list_of_pull_requests() {
    let home = TempHome::new();
    let engine = home.engine();

    for query in ["rektangel", "rektangel "] {
        let results = search_with(&engine, query, &Settings::default(), &[rektangel()]);

        let outcome = activate(&results, results.selected, Key::Enter);

        assert_eq!(
            outcome.action,
            url("https://github.com/pre/rektangel/pulls")
        );
    }
}

#[test]
fn r6_enter_on_a_shortcut_that_needs_an_argument_and_has_none_does_nothing() {
    let home = TempHome::new();
    let engine = home.engine();
    let results = search_with(&engine, "g", &Settings::default(), &[google()]);

    let outcome = activate(&results, results.selected, Key::Enter);

    assert_eq!(titles(&results)[0], "Google");
    assert_eq!(outcome.action, Action::Nothing);
    assert!(!outcome.close);
}

#[test]
fn r6_a_query_that_matches_nothing_shows_the_default_search_shortcuts_with_the_full_query() {
    let home = TempHome::new();
    let engine = home.engine();
    let shortcuts = [rektangel(), google()];

    let results = search_with(&engine, "zzq rust", &Settings::default(), &shortcuts);

    assert_eq!(results.mode, Mode::General);
    assert_eq!(titles(&results), ["Google"]);
    assert_eq!(
        results.rows[0].enter,
        url("https://www.google.com/search?q=zzq%20rust")
    );
}

#[test]
fn r6_the_default_search_shortcuts_do_not_show_when_a_row_matches() {
    let home = TempHome::new();
    let mut engine = home.engine();
    engine.register(Slot::Calculator, fake_calculator);

    let results = search_with(&engine, "42+1", &Settings::default(), &[google()]);

    assert_eq!(titles(&results), ["43"]);
}

#[test]
fn r6_the_keyword_ignores_letter_case() {
    let home = TempHome::new();
    let engine = home.engine();

    let results = search_with(&engine, "Rektangel 9", &Settings::default(), &[rektangel()]);

    assert_eq!(
        results.rows[0].enter,
        url("https://github.com/pre/rektangel/pull/9")
    );
}

#[test]
fn r6_a_shortcut_keyword_alone_comes_before_the_other_rows_of_the_general_search() {
    let home = TempHome::new();
    let mut engine = home.engine();
    engine.register(
        Slot::Applications,
        fake_applications(home.dir("applications")),
    );
    let files_shortcut = Shortcut {
        name: "Files of the team".into(),
        keyword: "files".into(),
        cmd: "https://files.example.com/".into(),
        run_without_argument: true,
        ..Shortcut::default()
    };

    let results = search_with(&engine, "files", &Settings::default(), &[files_shortcut]);

    assert_eq!(results.mode, Mode::General);
    assert_eq!(titles(&results), ["Files of the team", "Files"]);
    assert_eq!(results.selected, Some(0));
}

#[test]
fn r12_acceptance_a_url_shortcut_with_microsoft_edge_opens_the_url_in_microsoft_edge() {
    let home = TempHome::new();
    let engine = home.engine();
    let edge = Browser::App("microsoft-edge.desktop".into());
    let beacon = Shortcut {
        name: "Beacon".into(),
        keyword: "beacon".into(),
        cmd: "https://beacon.example.com/%s".into(),
        browser: Some(edge.clone()),
        ..Shortcut::default()
    };
    let results = search_with(&engine, "beacon 5", &Settings::default(), &[beacon]);

    let outcome = activate(&results, results.selected, Key::Enter);

    assert_eq!(
        outcome.action,
        Action::OpenUrl {
            url: "https://beacon.example.com/5".into(),
            browser: Some(edge),
        }
    );
}
