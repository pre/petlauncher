//! R4: the GNOME custom shortcut of petlauncher. Each test uses the memory
//! GSettings backend (see `support::memory_backend`), never dconf.

mod support;

use std::fs;

use gio::prelude::*;
use petlauncher_gnome::{
    CustomShortcut, CustomShortcuts, GnomeDesktop, SHORTCUT_NAME, SHORTCUT_PATH, TOGGLE_COMMAND,
};
use support::{SettingsFixture, KUKKA_PATH, ULAUNCHER_PATH};

fn shortcuts(fixture: &SettingsFixture) -> CustomShortcuts {
    CustomShortcuts::with_backend(&fixture.source, &fixture.backend).expect("test schemas")
}

fn ulauncher() -> CustomShortcut {
    CustomShortcut {
        path: ULAUNCHER_PATH.into(),
        name: "Show Ulauncher".into(),
        command: "ulauncher toggle".into(),
        binding: "<Super>space".into(),
    }
}

#[test]
fn has_no_global_key_before_the_first_installation() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();

    let key = shortcuts(&fixture).global_key();

    assert_eq!(key, None);
}

#[test]
fn set_global_key_writes_the_entry_and_adds_its_path_after_the_other_entries() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();

    shortcuts(&fixture)
        .set_global_key("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(fixture.paths(), [ULAUNCHER_PATH, KUKKA_PATH, SHORTCUT_PATH]);
    assert_eq!(
        fixture.read(SHORTCUT_PATH),
        ["Show petlauncher", "petlauncher toggle", "<Super>space"]
    );
    assert_eq!(SHORTCUT_NAME, "Show petlauncher");
}

#[test]
fn set_global_key_does_not_change_the_other_entries() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();

    shortcuts(&fixture)
        .set_global_key("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(
        fixture.read(ULAUNCHER_PATH),
        ["Show Ulauncher", "ulauncher toggle", "<Super>space"]
    );
    assert_eq!(fixture.read(KUKKA_PATH)[2], "<Control><Super>plus");
}

#[test]
fn global_key_gives_the_key_that_set_global_key_wrote() {
    let fixture = SettingsFixture::new();
    let shortcuts = shortcuts(&fixture);

    shortcuts
        .set_global_key("<Alt>space", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(shortcuts.global_key(), Some("<Alt>space".into()));
}

#[test]
fn a_second_set_global_key_changes_the_key_and_does_not_add_the_path_again() {
    let fixture = SettingsFixture::new();
    let shortcuts = shortcuts(&fixture);

    shortcuts
        .set_global_key("<Super>space", TOGGLE_COMMAND)
        .unwrap();
    shortcuts
        .set_global_key("<Alt>F1", "/opt/petlauncher toggle")
        .unwrap();

    assert_eq!(fixture.paths(), [SHORTCUT_PATH]);
    assert_eq!(
        fixture.read(SHORTCUT_PATH),
        ["Show petlauncher", "/opt/petlauncher toggle", "<Alt>F1"]
    );
}

#[test]
fn an_empty_accelerator_removes_the_key_and_keeps_the_entry() {
    let fixture = SettingsFixture::new();
    let shortcuts = shortcuts(&fixture);
    shortcuts
        .set_global_key("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    shortcuts.set_global_key("", TOGGLE_COMMAND).unwrap();

    assert_eq!(shortcuts.global_key(), None);
    assert_eq!(fixture.paths(), [SHORTCUT_PATH]);
    assert_eq!(fixture.read(SHORTCUT_PATH)[2], "");
}

#[test]
fn global_key_ignores_an_entry_that_is_not_in_the_list() {
    let fixture = SettingsFixture::new();
    fixture.write(SHORTCUT_PATH, SHORTCUT_NAME, TOGGLE_COMMAND, "<Super>space");

    let key = shortcuts(&fixture).global_key();

    assert_eq!(key, None);
}

#[test]
fn set_global_key_rejects_an_accelerator_that_is_not_correct_and_writes_nothing() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();

    let error = shortcuts(&fixture)
        .set_global_key("<Win>space", TOGGLE_COMMAND)
        .unwrap_err();

    assert!(error.0.contains("<Win> is not a modifier"), "{error}");
    assert_eq!(fixture.paths(), [ULAUNCHER_PATH, KUKKA_PATH]);
    assert_eq!(fixture.read(SHORTCUT_PATH), ["", "", ""]);
}

#[test]
fn others_with_key_finds_the_ulauncher_shortcut_for_super_space() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();

    let others = shortcuts(&fixture).others_with_key("<super>Space").unwrap();

    assert_eq!(others, [ulauncher()]);
}

#[test]
fn others_with_key_ignores_petlauncher_and_the_entries_that_are_not_in_the_list() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();
    let shortcuts = shortcuts(&fixture);
    shortcuts.set_global_key("<Alt>F2", TOGGLE_COMMAND).unwrap();
    fixture.write(
        "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/old/",
        "Old",
        "old",
        "<Alt>F2",
    );

    let others = shortcuts.others_with_key("<Alt>F2").unwrap();

    assert_eq!(others, []);
}

#[test]
fn others_with_key_skips_a_path_that_gsettings_cannot_open() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();
    fixture.set_paths(&["no-slash", "/double//slash/", ULAUNCHER_PATH]);

    let others = shortcuts(&fixture).others_with_key("<Super>space").unwrap();

    assert_eq!(others, [ulauncher()]);
}

#[test]
fn others_with_key_of_no_key_is_empty() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();
    fixture.write(KUKKA_PATH, "kukka", "kukka", "");

    assert_eq!(shortcuts(&fixture).others_with_key("").unwrap(), []);
}

#[test]
fn others_with_key_rejects_an_accelerator_that_is_not_correct() {
    let fixture = SettingsFixture::new();

    let error = shortcuts(&fixture).others_with_key("<Super>").unwrap_err();

    assert!(error.0.contains("no key after the modifiers"), "{error}");
}

#[test]
fn take_key_clears_the_binding_of_ulauncher_and_reports_the_entry() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();

    let taken = shortcuts(&fixture).take_key("<Super>space").unwrap();

    assert_eq!(taken, [ulauncher()]);
    assert_eq!(
        fixture.read(ULAUNCHER_PATH),
        ["Show Ulauncher", "ulauncher toggle", ""]
    );
    assert_eq!(fixture.paths(), [ULAUNCHER_PATH, KUKKA_PATH]);
    assert_eq!(fixture.read(KUKKA_PATH)[2], "<Control><Super>plus");
}

#[test]
fn take_key_changes_nothing_when_no_other_entry_has_the_key() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();

    let taken = shortcuts(&fixture).take_key("<Alt>space").unwrap();

    assert_eq!(taken, []);
    assert_eq!(fixture.read(ULAUNCHER_PATH)[2], "<Super>space");
}

#[test]
fn take_key_rejects_an_accelerator_that_is_not_correct_and_changes_nothing() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();

    assert!(shortcuts(&fixture).take_key("<Super>spcae").is_err());
    assert_eq!(fixture.read(ULAUNCHER_PATH)[2], "<Super>space");
}

#[test]
fn r4_first_installation_takes_super_space_from_ulauncher() {
    let fixture = SettingsFixture::new();
    fixture.owner_setup();
    let shortcuts = shortcuts(&fixture);

    let taken = shortcuts.take_key("<Super>space").unwrap();
    shortcuts
        .set_global_key("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(taken, [ulauncher()]);
    assert_eq!(shortcuts.global_key(), Some("<Super>space".into()));
    assert_eq!(shortcuts.others_with_key("<Super>space").unwrap(), []);
}

#[test]
fn a_missing_schema_gives_an_error() {
    let _backend = support::memory_backend();
    let dir = tempfile::tempdir().unwrap();
    let schemas = dir.path().join("schemas");
    fs::create_dir(&schemas).unwrap();
    fs::write(
        schemas.join("pet.repo.Other.gschema.xml"),
        r#"<schemalist><schema id="pet.repo.Other" path="/pet/repo/other/"><key name="a" type="b"><default>false</default></key></schema></schemalist>"#,
    )
    .unwrap();
    let status = std::process::Command::new("glib-compile-schemas")
        .arg(&schemas)
        .status()
        .unwrap();
    assert!(status.success());
    let source = gio::SettingsSchemaSource::from_directory(&schemas, None, true).unwrap();

    let error = CustomShortcuts::with_backend(&source, &support::memory_backend())
        .err()
        .expect("no media-keys schema");

    assert!(
        error
            .0
            .contains("org.gnome.settings-daemon.plugins.media-keys"),
        "{error}"
    );
}

/// Reads the installed schemas, and writes nothing.
#[test]
fn the_installed_schema_has_the_keys_of_the_test_schema() {
    let Some(installed) = gio::SettingsSchemaSource::default() else {
        eprintln!("no installed schemas: the test does nothing");
        return;
    };
    if installed.lookup(support::MEDIA_KEYS, true).is_none() {
        eprintln!("gnome-settings-daemon is not installed: the test does nothing");
        return;
    }
    let test = support::test_schemas();

    for (id, keys) in [
        (support::MEDIA_KEYS, &["custom-keybindings"][..]),
        (support::CUSTOM_KEYBINDING, &["name", "command", "binding"]),
    ] {
        let real = installed.lookup(id, true).unwrap();
        let copy = test.lookup(id, false).unwrap();
        assert_eq!(real.path(), copy.path(), "{id}");
        for key in keys {
            assert_eq!(
                real.key(key).value_type(),
                copy.key(key).value_type(),
                "{id} {key}"
            );
        }
    }
}

#[test]
fn the_desktop_gives_the_global_key_of_its_custom_shortcuts() {
    use petlauncher_core::Desktop;
    let fixture = SettingsFixture::new();
    let bus = support::PrivateBus::start();
    let desktop = GnomeDesktop::with_parts(bus.connect(), None, Ok(shortcuts(&fixture)));

    desktop
        .set_global_key("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(desktop.global_key(), Ok(Some("<Super>space".into())));
    assert_eq!(fixture.paths(), [SHORTCUT_PATH]);
    assert!(desktop.shortcuts().is_ok());
}

#[test]
fn the_desktop_tells_why_it_has_no_custom_shortcuts() {
    use petlauncher_core::{Desktop, DesktopError};
    let bus = support::PrivateBus::start();
    let desktop =
        GnomeDesktop::with_parts(bus.connect(), None, Err(DesktopError("no schema".into())));

    assert_eq!(desktop.global_key(), Err(DesktopError("no schema".into())));
    assert_eq!(
        desktop.set_global_key("<Super>space", TOGGLE_COMMAND),
        Err(DesktopError("no schema".into()))
    );
}

// ------------------------------------------------- separate process

/// R4 in a separate process whose default GSettings backend is the keyfile
/// backend with a file in the isolated root. The file starts with the two
/// custom shortcuts of the owner. The child uses `GnomeDesktop::new`, as the
/// program does.
#[test]
fn r4_first_installation_in_a_process_with_the_keyfile_backend() {
    let env = support::IsolatedEnv::new();
    let keyfile = env.path("config/glib-2.0/settings/keyfile");
    fs::create_dir_all(keyfile.parent().unwrap()).unwrap();
    fs::write(&keyfile, OWNER_KEYFILE).unwrap();

    let status = env.run_child(
        "isolated_child",
        "first-installation",
        &[
            ("GSETTINGS_BACKEND", "keyfile"),
            (
                "GSETTINGS_SCHEMA_DIR",
                support::test_schema_dir().to_str().unwrap(),
            ),
        ],
    );

    let output = env.output("first-installation");
    assert!(status.success(), "{output}");
    assert!(
        output.contains("backend GKeyfileSettingsBackend"),
        "{output}"
    );
    assert!(
        output.contains(&format!("taken {ULAUNCHER_PATH} <Super>space")),
        "{output}"
    );
    assert!(
        output.contains("global key Some(\"<Super>space\")"),
        "{output}"
    );
    let file = glib::KeyFile::new();
    file.load_from_file(&keyfile, glib::KeyFileFlags::NONE)
        .unwrap();
    let value = |group: &str, key: &str| file.value(group, key).unwrap().to_string();
    let base = "org/gnome/settings-daemon/plugins/media-keys";
    assert_eq!(
        value(base, "custom-keybindings"),
        format!("['{ULAUNCHER_PATH}', '{KUKKA_PATH}', '{SHORTCUT_PATH}']")
    );
    assert_eq!(
        value(&format!("{base}/custom-keybindings/ulauncher"), "binding"),
        "''"
    );
    assert_eq!(
        value(
            &format!("{base}/custom-keybindings/kukka-hotkey-window"),
            "binding"
        ),
        "'<Control><Super>plus'"
    );
    let petlauncher = format!("{base}/custom-keybindings/petlauncher");
    assert_eq!(value(&petlauncher, "binding"), "'<Super>space'");
    assert_eq!(value(&petlauncher, "command"), "'petlauncher toggle'");
    assert_eq!(value(&petlauncher, "name"), "'Show petlauncher'");
}

const OWNER_KEYFILE: &str = "[org/gnome/settings-daemon/plugins/media-keys]
custom-keybindings=['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/ulauncher/', '/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/kukka-hotkey-window/']

[org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/ulauncher]
binding='<Super>space'
command='ulauncher toggle'
name='Show Ulauncher'

[org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/kukka-hotkey-window]
binding='<Control><Super>plus'
command='gapplication action dev.kukka.Kukka toggle-quick-terminal'
name='kukka Hotkey Window'
";

/// The child side of the tests above. It does nothing outside `run_child`.
#[test]
#[ignore = "runs only in the isolated child process of another test"]
fn isolated_child() {
    use petlauncher_core::Desktop;
    let Some((scenario, root)) = support::child_scenario() else {
        return;
    };
    assert_eq!(scenario, "first-installation");
    let backend = gio::SettingsBackend::default();
    let file: String = backend.property("filename");
    assert!(file.starts_with(root.to_str().unwrap()), "{file}");
    println!("backend {}", backend.type_().name());

    let desktop = GnomeDesktop::new().unwrap();
    let taken = desktop
        .shortcuts()
        .unwrap()
        .take_key("<Super>space")
        .unwrap();
    desktop
        .set_global_key("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    for shortcut in taken {
        println!("taken {} {}", shortcut.path, shortcut.binding);
    }
    println!("global key {:?}", desktop.global_key().unwrap());
}
