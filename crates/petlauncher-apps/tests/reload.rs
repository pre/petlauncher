//! The change check and the reload.

mod common;

use std::fs;

use common::{app, Fixture};
use petlauncher_apps::{gnome, Applications};

fn loaded_fixture() -> (Fixture, Applications) {
    let fixture = Fixture::new();
    fixture.program("x");
    fixture.write(&fixture.system(), "x.desktop", &app("X", "x", ""));
    fixture.write(&fixture.system(), "kde4/y.desktop", &app("Y", "x", ""));
    fixture.make_old();
    let apps = Applications::load(fixture.env(), gnome::is_settings_page);
    (fixture, apps)
}

#[test]
fn reports_no_change_after_a_load() {
    let (_fixture, apps) = loaded_fixture();

    assert!(!apps.has_changed());
}

#[test]
fn reports_a_change_when_a_file_is_added() {
    let (fixture, apps) = loaded_fixture();

    fixture.write(&fixture.system(), "new.desktop", &app("New", "x", ""));

    assert!(apps.has_changed());
}

#[test]
fn reports_a_change_when_a_file_is_added_in_a_subdirectory() {
    let (fixture, apps) = loaded_fixture();

    fixture.write(&fixture.system(), "kde4/z.desktop", &app("Z", "x", ""));

    assert!(apps.has_changed());
}

#[test]
fn reports_a_change_when_a_file_is_removed() {
    let (fixture, apps) = loaded_fixture();

    fs::remove_file(fixture.system().join("applications/x.desktop")).unwrap();

    assert!(apps.has_changed());
}

#[test]
fn reports_a_change_when_the_text_of_a_file_changes() {
    let (fixture, apps) = loaded_fixture();

    fs::write(
        fixture.system().join("applications/x.desktop"),
        app("X changed", "x", ""),
    )
    .unwrap();

    assert!(apps.has_changed());
}

#[test]
fn reports_a_change_when_a_missing_applications_directory_is_made() {
    let (fixture, apps) = loaded_fixture();

    fs::create_dir_all(fixture.user().join("applications")).unwrap();

    assert!(apps.has_changed());
}

#[test]
fn reads_the_new_entries_at_a_reload() {
    let (fixture, apps) = loaded_fixture();
    fixture.write(&fixture.user(), "new.desktop", &app("New", "x", ""));

    let reloaded = apps.reload();

    assert!(reloaded.get("new.desktop").is_some());
    assert!(apps.get("new.desktop").is_none());
    assert!(!reloaded.has_changed());
}
