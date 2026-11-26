//! R13: Shift+Enter copies the path of the local file of the row.

mod common;

use common::{fake_applications, search, TempHome};
use petlauncher_core::{activate, Action, Key, Slot};

#[test]
fn r13_acceptance_shift_enter_on_the_row_of_a_pasted_path_copies_the_full_path_and_closes() {
    let home = TempHome::new();
    let file = home.file(
        "lolcat-evidence/kukka/font-size-keys-palette/screenshots-final/palette-zoom.png",
        1,
    );
    let engine = home.engine();
    let results = search(&engine, file.to_str().unwrap());

    let outcome = activate(&results, results.selected, Key::ShiftEnter);

    assert_eq!(
        outcome.action,
        Action::Copy(file.to_str().unwrap().to_owned())
    );
    assert!(outcome.close);
    assert!(!outcome.keep_query);
}

#[test]
fn r13_shift_enter_on_an_application_row_copies_the_path_of_its_desktop_file() {
    let home = TempHome::new();
    let applications = home.dir("applications");
    let mut engine = home.engine();
    engine.register(Slot::Applications, fake_applications(applications.clone()));
    let results = search(&engine, "firefox");

    let outcome = activate(&results, results.selected, Key::ShiftEnter);

    let desktop_file = applications.join("firefox.desktop");
    assert_eq!(
        outcome.action,
        Action::Copy(desktop_file.to_str().unwrap().to_owned())
    );
    assert!(outcome.close);
}

#[test]
fn r13_shift_enter_with_the_query_as_a_path_with_a_tilde_copies_the_full_path() {
    let home = TempHome::new();
    let file = home.file("notes.txt", 1);
    let engine = home.engine();
    let results = search(&engine, "~/notes.txt");

    let outcome = activate(&results, results.selected, Key::ShiftEnter);

    assert_eq!(
        outcome.action,
        Action::Copy(file.to_str().unwrap().to_owned())
    );
}
