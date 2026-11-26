//! R4: the GNOME custom shortcut of petlauncher. Each test uses the memory
//! GSettings backend (see `support::memory_backend`), never dconf.

mod support;

use std::fs;

use gio::prelude::*;
use petlauncher_gnome::{
    BindingKind, Conflict, CustomShortcut, CustomShortcuts, GnomeDesktop, CUSTOM_SECTION,
    SHORTCUT_NAME, SHORTCUT_PATH, TOGGLE_COMMAND,
};
use support::{SettingsFixture, NOTES_PATH, TERMINAL_PATH, WM_KEYBINDINGS};

fn shortcuts(fixture: &SettingsFixture) -> CustomShortcuts {
    CustomShortcuts::with_backend(&fixture.source, &fixture.backend).expect("test schemas")
}

/// The shortcuts and the built-in bindings with the names of GNOME Settings.
fn named_shortcuts(fixture: &SettingsFixture) -> CustomShortcuts {
    shortcuts(fixture).with_catalog_dirs(vec![support::catalog_dir()])
}

fn notes() -> CustomShortcut {
    CustomShortcut {
        path: NOTES_PATH.into(),
        name: "Show Scratch Notes".into(),
        command: "scratch-notes --toggle".into(),
        binding: "<Super>space".into(),
    }
}

#[test]
fn has_no_global_key_before_the_first_installation() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();

    let key = shortcuts(&fixture).global_key();

    assert_eq!(key, None);
}

#[test]
fn set_global_key_writes_the_entry_and_adds_its_path_after_the_other_entries() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();

    shortcuts(&fixture)
        .set_global_key("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(fixture.paths(), [NOTES_PATH, TERMINAL_PATH, SHORTCUT_PATH]);
    assert_eq!(
        fixture.read(SHORTCUT_PATH),
        ["Show petlauncher", "petlauncher toggle", "<Super>space"]
    );
    assert_eq!(SHORTCUT_NAME, "Show petlauncher");
}

#[test]
fn set_global_key_does_not_change_the_other_entries() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();

    shortcuts(&fixture)
        .set_global_key("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(
        fixture.read(NOTES_PATH),
        [
            "Show Scratch Notes",
            "scratch-notes --toggle",
            "<Super>space"
        ]
    );
    assert_eq!(fixture.read(TERMINAL_PATH)[2], "<Control><Super>plus");
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
    fixture.two_shortcuts();

    let error = shortcuts(&fixture)
        .set_global_key("<Win>space", TOGGLE_COMMAND)
        .unwrap_err();

    assert!(error.0.contains("<Win> is not a modifier"), "{error}");
    assert_eq!(fixture.paths(), [NOTES_PATH, TERMINAL_PATH]);
    assert_eq!(fixture.read(SHORTCUT_PATH), ["", "", ""]);
}

#[test]
fn others_with_key_finds_the_custom_shortcut_with_super_space() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();

    let others = shortcuts(&fixture).others_with_key("<super>Space").unwrap();

    assert_eq!(others, [notes()]);
}

#[test]
fn others_with_key_ignores_petlauncher_and_the_entries_that_are_not_in_the_list() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();
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
    fixture.two_shortcuts();
    fixture.set_paths(&["no-slash", "/double//slash/", NOTES_PATH]);

    let others = shortcuts(&fixture).others_with_key("<Super>space").unwrap();

    assert_eq!(others, [notes()]);
}

#[test]
fn others_with_key_of_no_key_is_empty() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();
    fixture.write(TERMINAL_PATH, "Quick Terminal", "quick-terminal", "");

    assert_eq!(shortcuts(&fixture).others_with_key("").unwrap(), []);
}

#[test]
fn others_with_key_rejects_an_accelerator_that_is_not_correct() {
    let fixture = SettingsFixture::new();

    let error = shortcuts(&fixture).others_with_key("<Super>").unwrap_err();

    assert!(error.0.contains("no key after the modifiers"), "{error}");
}

fn built_in(schema: &str, key: &str, name: &str, section: Option<&str>, stored: &str) -> Conflict {
    Conflict {
        kind: BindingKind::BuiltIn {
            schema: schema.into(),
            key: key.into(),
        },
        name: name.into(),
        section: section.map(Into::into),
        accelerator: stored.into(),
    }
}

fn strings(settings: &gio::Settings, key: &str) -> Vec<String> {
    settings
        .strv(key)
        .iter()
        .map(|text| text.to_string())
        .collect()
}

fn custom_conflict() -> Conflict {
    Conflict {
        kind: BindingKind::Custom {
            path: NOTES_PATH.into(),
        },
        name: "Show Scratch Notes".into(),
        section: Some(CUSTOM_SECTION.into()),
        accelerator: "<Super>space".into(),
    }
}

#[test]
fn conflicts_give_a_custom_shortcut_with_the_section_of_gnome_settings() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();

    let conflicts = named_shortcuts(&fixture).conflicts("<Super>space").unwrap();

    assert_eq!(conflicts, [custom_conflict()]);
    assert_eq!(
        conflicts[0].place(),
        "GNOME Settings > Keyboard > View and Customize Shortcuts > Custom Shortcuts"
    );
}

#[test]
fn conflicts_give_the_input_source_switch_of_gnome_with_its_name_in_gnome_settings() {
    let fixture = SettingsFixture::new();

    let conflicts = named_shortcuts(&fixture).conflicts("<Super>space").unwrap();

    assert_eq!(
        conflicts,
        [built_in(
            WM_KEYBINDINGS,
            "switch-input-source",
            "Switch to next input source",
            Some("Typing"),
            "<Super>space"
        )]
    );
}

#[test]
fn conflicts_compare_the_normal_form_of_the_key() {
    let fixture = SettingsFixture::new();
    fixture
        .built_in(WM_KEYBINDINGS)
        .set_strv("switch-input-source", ["XF86Keyboard", "<Mod4>space"])
        .unwrap();
    fixture
        .built_in(WM_KEYBINDINGS)
        .set_strv("activate-window-menu", ["<Shift><Control>x"])
        .unwrap();

    let shortcuts = named_shortcuts(&fixture);
    let super_space = shortcuts.conflicts("<super>Space").unwrap();
    let control_shift = shortcuts.conflicts("<Control><Shift>X").unwrap();

    assert_eq!(super_space.len(), 1);
    assert_eq!(super_space[0].accelerator, "<Mod4>space");
    assert_eq!(control_shift.len(), 1);
    assert_eq!(control_shift[0].accelerator, "<Shift><Control>x");
}

#[test]
fn a_key_that_gnome_settings_does_not_show_gets_the_summary_of_its_schema() {
    let fixture = SettingsFixture::new();
    fixture
        .built_in("org.gnome.shell.keybindings")
        .set_strv("toggle-overview", ["<Super>F9"])
        .unwrap();

    let conflicts = named_shortcuts(&fixture).conflicts("<Super>F9").unwrap();

    assert_eq!(
        conflicts,
        [built_in(
            "org.gnome.shell.keybindings",
            "toggle-overview",
            "Show the overview",
            None,
            "<Super>F9"
        )]
    );
}

#[test]
fn conflicts_skip_the_list_of_custom_shortcuts_a_number_and_petlauncher() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();
    let shortcuts = named_shortcuts(&fixture);
    shortcuts
        .set_global_key("<Control><Alt>t", TOGGLE_COMMAND)
        .unwrap();

    let conflicts = shortcuts.conflicts("<Primary><Alt>t").unwrap();

    assert_eq!(
        conflicts,
        [built_in(
            support::MEDIA_KEYS,
            "terminal",
            "Launch terminal",
            Some("Launchers"),
            "<Primary><Alt>t"
        )]
    );
}

#[test]
fn no_key_has_no_conflict_and_a_wrong_key_is_an_error() {
    let fixture = SettingsFixture::new();
    let shortcuts = named_shortcuts(&fixture);

    assert_eq!(shortcuts.conflicts("").unwrap(), []);
    assert_eq!(shortcuts.conflicts("disabled").unwrap(), []);
    assert!(shortcuts.conflicts("<Win>space").is_err());
}

#[test]
fn set_global_key_if_free_writes_nothing_when_a_different_binding_has_the_key() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();
    let shortcuts = named_shortcuts(&fixture);

    let conflicts = shortcuts
        .set_global_key_if_free("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(conflicts, [custom_conflict()]);
    assert_eq!(shortcuts.global_key(), None);
    assert_eq!(fixture.paths(), [NOTES_PATH, TERMINAL_PATH]);
}

#[test]
fn set_global_key_if_free_writes_the_key_when_no_binding_has_it() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();
    let shortcuts = named_shortcuts(&fixture);

    let conflicts = shortcuts
        .set_global_key_if_free("<Super>F12", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(conflicts, []);
    assert_eq!(shortcuts.global_key(), Some("<Super>F12".into()));
}

#[test]
fn remove_key_of_a_custom_shortcut_keeps_the_entry_with_its_name_and_command() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();
    let shortcuts = named_shortcuts(&fixture);

    shortcuts.remove_key(&custom_conflict()).unwrap();

    assert_eq!(
        fixture.read(NOTES_PATH),
        ["Show Scratch Notes", "scratch-notes --toggle", ""]
    );
    assert_eq!(fixture.paths(), [NOTES_PATH, TERMINAL_PATH]);
    assert_eq!(fixture.read(TERMINAL_PATH)[2], "<Control><Super>plus");
    assert_eq!(shortcuts.conflicts("<Super>space").unwrap(), []);
}

#[test]
fn remove_key_of_a_built_in_binding_keeps_its_other_accelerators() {
    let fixture = SettingsFixture::new();
    let shortcuts = named_shortcuts(&fixture);
    let conflict = shortcuts.conflicts("<Super>space").unwrap().remove(0);

    shortcuts.remove_key(&conflict).unwrap();

    assert_eq!(
        strings(&fixture.built_in(WM_KEYBINDINGS), "switch-input-source"),
        ["XF86Keyboard"]
    );
    assert_eq!(
        strings(
            &fixture.built_in(WM_KEYBINDINGS),
            "switch-input-source-backward"
        ),
        ["<Shift><Super>space", "<Shift>XF86Keyboard"]
    );
    assert_eq!(shortcuts.conflicts("<Super>space").unwrap(), []);
}

#[test]
fn after_remove_key_the_global_key_is_free() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();
    let shortcuts = named_shortcuts(&fixture);
    for conflict in shortcuts.conflicts("<Super>space").unwrap() {
        shortcuts.remove_key(&conflict).unwrap();
    }

    let conflicts = shortcuts
        .set_global_key_if_free("<Super>space", TOGGLE_COMMAND)
        .unwrap();

    assert_eq!(conflicts, []);
    assert_eq!(shortcuts.global_key(), Some("<Super>space".into()));
}

#[test]
fn remove_key_rejects_a_binding_that_is_not_a_gnome_key_binding() {
    let fixture = SettingsFixture::new();
    fixture.two_shortcuts();
    let shortcuts = named_shortcuts(&fixture);
    let wrong = [
        built_in("org.example.Other", "key", "Other", None, "<Super>space"),
        built_in(
            support::MEDIA_KEYS,
            "custom-keybindings",
            "List",
            None,
            NOTES_PATH,
        ),
        built_in(support::MEDIA_KEYS, "volume-step", "Step", None, "6"),
        Conflict {
            kind: BindingKind::Custom {
                path: SHORTCUT_PATH.into(),
            },
            ..custom_conflict()
        },
    ];

    for conflict in &wrong {
        assert!(shortcuts.remove_key(conflict).is_err(), "{conflict:?}");
    }
    assert_eq!(fixture.paths(), [NOTES_PATH, TERMINAL_PATH]);
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
        (
            support::MEDIA_KEYS,
            &["custom-keybindings", "terminal", "volume-step"][..],
        ),
        (support::CUSTOM_KEYBINDING, &["name", "command", "binding"]),
        (
            WM_KEYBINDINGS,
            &[
                "switch-input-source",
                "switch-input-source-backward",
                "activate-window-menu",
            ],
        ),
        ("org.gnome.shell.keybindings", &["toggle-overview"]),
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

/// The installed schemas and files of GNOME Settings, with the memory
/// backend: the GNOME default of the input source switch is Super+Space,
/// and GNOME Settings names it. Reads the installed files, and writes
/// nothing.
#[test]
fn the_installed_files_of_gnome_settings_name_the_default_input_source_key() {
    let backend = support::memory_backend();
    let Some(installed) = gio::SettingsSchemaSource::default() else {
        eprintln!("no installed schemas: the test does nothing");
        return;
    };
    let Ok(shortcuts) = CustomShortcuts::with_backend(&installed, &backend) else {
        eprintln!("gnome-settings-daemon is not installed: the test does nothing");
        return;
    };
    let catalog = std::path::PathBuf::from("/usr/share/gnome-control-center/keybindings");
    if !catalog.is_dir() {
        eprintln!("GNOME Settings is not installed: the test does nothing");
        return;
    }

    let conflicts = shortcuts
        .with_catalog_dirs(vec![catalog])
        .conflicts("<Super>space")
        .unwrap();

    assert!(
        conflicts.contains(&built_in(
            WM_KEYBINDINGS,
            "switch-input-source",
            "Switch to next input source",
            Some("Typing"),
            "<Super>space"
        )),
        "{conflicts:#?}"
    );
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
/// backend with a file in the isolated root. The file has two custom
/// shortcuts of other programs, one with Super+Space. The child uses
/// `GnomeDesktop::new`, as the program does: the first write is refused for
/// the conflict, and after `remove_key`, which only the owner starts, the
/// key is written.
#[test]
fn r4_conflict_and_removal_in_a_process_with_the_keyfile_backend() {
    let env = support::IsolatedEnv::new();
    let keyfile = env.path("config/glib-2.0/settings/keyfile");
    fs::create_dir_all(keyfile.parent().unwrap()).unwrap();
    fs::write(&keyfile, TWO_SHORTCUTS_KEYFILE).unwrap();

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
        output.contains(&format!("conflict {NOTES_PATH} <Super>space")),
        "{output}"
    );
    assert!(
        output.contains("global key after the conflict None"),
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
        format!("['{NOTES_PATH}', '{TERMINAL_PATH}', '{SHORTCUT_PATH}']")
    );
    let notes = format!("{base}/custom-keybindings/scratch-notes");
    assert_eq!(value(&notes, "binding"), "''");
    assert_eq!(value(&notes, "name"), "'Show Scratch Notes'");
    assert_eq!(
        value(
            &format!("{base}/custom-keybindings/quick-terminal"),
            "binding"
        ),
        "'<Control><Super>plus'"
    );
    let petlauncher = format!("{base}/custom-keybindings/petlauncher");
    assert_eq!(value(&petlauncher, "binding"), "'<Super>space'");
    assert_eq!(value(&petlauncher, "command"), "'petlauncher toggle'");
    assert_eq!(value(&petlauncher, "name"), "'Show petlauncher'");
}

const TWO_SHORTCUTS_KEYFILE: &str = "[org/gnome/settings-daemon/plugins/media-keys]
custom-keybindings=['/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/scratch-notes/', '/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/quick-terminal/']

[org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/scratch-notes]
binding='<Super>space'
command='scratch-notes --toggle'
name='Show Scratch Notes'

[org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/quick-terminal]
binding='<Control><Super>plus'
command='quick-terminal'
name='Quick Terminal'

[org/gnome/desktop/wm/keybindings]
switch-input-source=@as []
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
    let shortcuts = desktop.shortcuts().unwrap();
    let conflicts = shortcuts
        .set_global_key_if_free("<Super>space", TOGGLE_COMMAND)
        .unwrap();
    for conflict in &conflicts {
        if let BindingKind::Custom { path } = &conflict.kind {
            println!("conflict {path} {}", conflict.accelerator);
        }
    }
    println!(
        "global key after the conflict {:?}",
        desktop.global_key().unwrap()
    );
    for conflict in &conflicts {
        shortcuts.remove_key(conflict).unwrap();
    }
    let left = shortcuts
        .set_global_key_if_free("<Super>space", TOGGLE_COMMAND)
        .unwrap();
    assert_eq!(left, []);
    println!("global key {:?}", desktop.global_key().unwrap());
}
