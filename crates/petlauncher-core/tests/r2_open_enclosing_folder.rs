//! R2: Ctrl+Enter opens the enclosing folder of any local file.

mod common;

use common::{fake_applications, fake_calculator, search, TempHome};
use petlauncher_core::{activate, Action, Key, RowKind, Slot};

#[test]
fn r2_step1_ctrl_enter_on_an_application_shows_its_desktop_file_and_does_not_start_it() {
    let home = TempHome::new();
    let applications = home.dir("applications");
    let mut engine = home.engine();
    engine.register(Slot::Applications, fake_applications(applications.clone()));
    let results = search(&engine, "Firefox");

    let outcome = activate(&results, results.selected, Key::CtrlEnter);

    let desktop_file = applications.join("firefox.desktop");
    assert_eq!(results.rows[0].kind, RowKind::Application);
    assert_eq!(outcome.action, Action::ShowInFolder(desktop_file.clone()));
    assert_ne!(outcome.action, Action::LaunchDesktopEntry(desktop_file));
    assert!(outcome.close);
}

#[test]
fn r2_step2_ctrl_enter_on_a_calculation_does_nothing() {
    let home = TempHome::new();
    let mut engine = home.engine();
    engine.register(Slot::Calculator, fake_calculator);
    let results = search(&engine, "42+1");

    let outcome = activate(&results, results.selected, Key::CtrlEnter);

    assert_eq!(results.rows[0].kind, RowKind::Calculation);
    assert_eq!(outcome.action, Action::Nothing);
    assert!(!outcome.close);
}

#[test]
fn r2_table_ctrl_enter_on_a_url_shortcut_does_nothing() {
    let home = TempHome::new();
    let engine = home.engine();
    let shortcuts = [petlauncher_core::config::Shortcut {
        name: "Google".into(),
        keyword: "g".into(),
        cmd: "https://www.google.com/search?q=%s".into(),
        ..Default::default()
    }];
    let results = common::search_with(
        &engine,
        "g rust",
        &petlauncher_core::config::Settings::default(),
        &shortcuts,
    );

    let outcome = activate(&results, results.selected, Key::CtrlEnter);

    assert_eq!(results.rows[0].kind, RowKind::Shortcut);
    assert_eq!(outcome.action, Action::Nothing);
}
