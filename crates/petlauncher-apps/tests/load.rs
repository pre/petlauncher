//! Directories, desktop file IDs, filters, settings pages and browsers.

mod common;

use std::fs;
use std::os::unix::fs::symlink;

use common::{app, Fixture};
use petlauncher_apps::{gnome, Applications, DesktopEntry, EntryKind};

fn ids(apps: &Applications) -> Vec<&str> {
    apps.entries()
        .iter()
        .map(|entry| entry.id.as_str())
        .collect()
}

fn no_pages(_: &DesktopEntry) -> bool {
    false
}

#[test]
fn finds_entries_in_all_data_directories_with_their_full_paths() {
    let fixture = Fixture::new();
    fixture.program("gnome-terminal");
    fixture.program("chatgpt");
    let system = fixture.write(
        &fixture.system(),
        "org.gnome.Terminal.desktop",
        &app("Terminal", "gnome-terminal", ""),
    );
    let user = fixture.write(
        &fixture.user(),
        "chatgpt.desktop",
        &app("ChatGPT", "chatgpt %U", ""),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(
        ids(&apps),
        ["chatgpt.desktop", "org.gnome.Terminal.desktop"]
    );
    assert_eq!(apps.get("org.gnome.Terminal.desktop").unwrap().path, system);
    assert_eq!(apps.get("chatgpt.desktop").unwrap().path, user);
    assert!(apps.errors().is_empty());
}

#[test]
fn makes_the_id_of_a_file_in_a_subdirectory_with_dashes() {
    let fixture = Fixture::new();
    fixture.program("kate");
    fixture.write(
        &fixture.system(),
        "kde4/org/kate.desktop",
        &app("Kate", "kate", ""),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(ids(&apps), ["kde4-org-kate.desktop"]);
}

#[test]
fn ignores_files_that_are_not_desktop_files() {
    let fixture = Fixture::new();
    fixture.program("x");
    fixture.write(&fixture.system(), "mimeinfo.cache", "[MIME Cache]\n");
    fixture.write(
        &fixture.system(),
        "defaults.list",
        &app("Not an entry", "x", ""),
    );
    fixture.write(&fixture.system(), "x.desktop", &app("X", "x", ""));

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(ids(&apps), ["x.desktop"]);
}

#[test]
fn lets_an_entry_in_an_earlier_directory_hide_the_entry_with_the_same_id() {
    let fixture = Fixture::new();
    fixture.program("bitwarden");
    fixture.write(
        &fixture.system(),
        "bitwarden_bitwarden.desktop",
        &app("Bitwarden", "bitwarden", ""),
    );
    let user = fixture.write(
        &fixture.user(),
        "bitwarden_bitwarden.desktop",
        &app("My Bitwarden", "bitwarden", ""),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    let entry = apps.get("bitwarden_bitwarden.desktop").unwrap();
    assert_eq!(entry.name, "My Bitwarden");
    assert_eq!(entry.path, user);
    assert_eq!(apps.entries().len(), 1);
}

#[test]
fn lets_a_hidden_entry_in_the_user_directory_remove_the_system_entry() {
    let fixture = Fixture::new();
    fixture.program("vim");
    fixture.write(&fixture.system(), "vim.desktop", &app("Vim", "vim %F", ""));
    fixture.write(
        &fixture.user(),
        "vim.desktop",
        &app("Vim", "vim %F", "Hidden=true\n"),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert!(apps.entries().is_empty());
}

#[test]
fn reports_a_file_that_cannot_be_read_and_lets_it_hide_the_later_entry() {
    let fixture = Fixture::new();
    fixture.program("vim");
    fixture.write(&fixture.system(), "vim.desktop", &app("Vim", "vim %F", ""));
    let broken = fixture.write(&fixture.user(), "vim.desktop", "Name=Vim without a group\n");

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert!(apps.entries().is_empty());
    assert_eq!(apps.errors().len(), 1);
    assert_eq!(apps.errors()[0].path, broken);
}

#[test]
fn excludes_other_types_no_display_and_hidden_entries() {
    let fixture = Fixture::new();
    fixture.program("x");
    let system = fixture.system();
    fixture.write(&system, "shown.desktop", &app("Shown", "x", ""));
    fixture.write(
        &system,
        "link.desktop",
        "[Desktop Entry]\nType=Link\nName=Link\nURL=https://example.com\n",
    );
    fixture.write(
        &system,
        "service.desktop",
        "[Desktop Entry]\nType=Service\nName=Service\nExec=x\n",
    );
    fixture.write(
        &system,
        "nodisplay.desktop",
        &app("No display", "x", "NoDisplay=true\n"),
    );
    fixture.write(
        &system,
        "hidden.desktop",
        &app("Hidden", "x", "Hidden=true\n"),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(ids(&apps), ["shown.desktop"]);
    assert!(apps.errors().is_empty());
}

#[test]
fn obeys_only_show_in_and_not_show_in_for_the_current_desktops() {
    let fixture = Fixture::new();
    fixture.program("x");
    let system = fixture.system();
    fixture.write(
        &system,
        "ubuntu-only.desktop",
        &app("Ubuntu only", "x", "OnlyShowIn=ubuntu;\n"),
    );
    fixture.write(
        &system,
        "kde-only.desktop",
        &app("KDE only", "x", "OnlyShowIn=KDE;\n"),
    );
    fixture.write(
        &system,
        "not-gnome.desktop",
        &app("Not GNOME", "x", "NotShowIn=GNOME;\n"),
    );
    fixture.write(
        &system,
        "not-kde.desktop",
        &app("Not KDE", "x", "NotShowIn=KDE;\n"),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(ids(&apps), ["not-kde.desktop", "ubuntu-only.desktop"]);
}

#[test]
fn excludes_an_entry_whose_try_exec_program_is_missing() {
    let fixture = Fixture::new();
    fixture.program("x");
    let evince = fixture.program("evince");
    let system = fixture.system();
    fixture.write(
        &system,
        "present.desktop",
        &app("Present", "x", "TryExec=x\n"),
    );
    fixture.write(
        &system,
        "absolute.desktop",
        &app("Absolute", "x", &format!("TryExec={}\n", evince.display())),
    );
    fixture.write(
        &system,
        "missing.desktop",
        &app("Missing", "x", "TryExec=not-installed\n"),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(ids(&apps), ["absolute.desktop", "present.desktop"]);
}

#[test]
fn excludes_an_entry_whose_exec_program_is_missing_or_not_executable() {
    let fixture = Fixture::new();
    fixture.program("present");
    fs::write(fixture.bin().join("not-executable"), "").unwrap();
    let system = fixture.system();
    fixture.write(
        &system,
        "present.desktop",
        &app("Present", "present %U", ""),
    );
    fixture.write(&system, "missing.desktop", &app("Missing", "texdoctk", ""));
    fixture.write(
        &system,
        "plain.desktop",
        &app("Plain file", "not-executable", ""),
    );
    fixture.write(
        &system,
        "no-exec.desktop",
        "[Desktop Entry]\nType=Application\nName=No exec\nDBusActivatable=true\n",
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(ids(&apps), ["no-exec.desktop", "present.desktop"]);
}

#[test]
fn reads_a_desktop_file_through_a_symbolic_link() {
    let fixture = Fixture::new();
    fixture.program("kukka");
    let target = fixture.root.join("app/dev.kukka.Kukka.desktop");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, app("kukka", "kukka", "")).unwrap();
    let applications = fixture.user().join("applications");
    fs::create_dir_all(&applications).unwrap();
    let link = applications.join("dev.kukka.Kukka.desktop");
    symlink(&target, &link).unwrap();

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(apps.get("dev.kukka.Kukka.desktop").unwrap().path, link);
}

#[test]
fn reads_a_directory_link_that_points_to_its_own_parent_only_once() {
    let fixture = Fixture::new();
    fixture.program("x");
    fixture.write(&fixture.system(), "x.desktop", &app("X", "x", ""));
    let applications = fixture.system().join("applications");
    symlink(&applications, applications.join("loop")).unwrap();

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(ids(&apps), ["x.desktop"]);
}

#[test]
fn uses_the_localized_values_of_the_environment_locale() {
    let fixture = Fixture::new();
    fixture.program("nautilus");
    fixture.write(
        &fixture.system(),
        "org.gnome.Nautilus.desktop",
        &app("Files", "nautilus", "Name[fi]=Tiedostot\n"),
    );
    let mut env = fixture.env();
    env.locale = Some("fi_FI.UTF-8".into());

    let apps = Applications::load(env, gnome::is_settings_page);

    assert_eq!(
        apps.get("org.gnome.Nautilus.desktop").unwrap().name,
        "Tiedostot"
    );
}

#[test]
fn includes_a_gnome_settings_page_that_has_no_display_as_a_settings_page() {
    let fixture = Fixture::new();
    fixture.program("gnome-control-center");
    let system = fixture.system();
    let wifi = "[Desktop Entry]\nType=Application\nName=Wi-Fi\nExec=gnome-control-center wifi\n\
                NoDisplay=true\nCategories=GNOME;GTK;Settings;X-GNOME-Settings-Panel;\n\
                OnlyShowIn=GNOME;Unity;\n";
    fixture.write(&system, "gnome-wifi-panel.desktop", wifi);
    fixture.write(
        &system,
        "org.gnome.Settings.desktop",
        &app("Settings", "gnome-control-center", "OnlyShowIn=GNOME;\n"),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    assert_eq!(
        apps.get("gnome-wifi-panel.desktop").unwrap().kind,
        EntryKind::SettingsPage
    );
    assert_eq!(
        apps.get("org.gnome.Settings.desktop").unwrap().kind,
        EntryKind::Application
    );
}

#[test]
fn excludes_the_settings_pages_with_a_rule_that_finds_none() {
    let fixture = Fixture::new();
    fixture.program("gnome-control-center");
    fixture.write(
        &fixture.system(),
        "gnome-wifi-panel.desktop",
        &app("Wi-Fi", "gnome-control-center wifi", "NoDisplay=true\n"),
    );

    let apps = Applications::load(fixture.env(), no_pages);

    assert!(apps.entries().is_empty());
}

#[test]
fn applies_the_show_in_rules_to_settings_pages_also() {
    let fixture = Fixture::new();
    fixture.program("gnome-control-center");
    let page = app(
        "Ubuntu Desktop",
        "gnome-control-center ubuntu",
        "NoDisplay=true\nOnlyShowIn=ubuntu;\n",
    );
    fixture.write(&fixture.system(), "gnome-ubuntu-panel.desktop", &page);
    let mut env = fixture.env();
    env.current_desktops = vec!["GNOME".into()];

    let apps = Applications::load(env, gnome::is_settings_page);

    assert!(apps.entries().is_empty());
}

#[test]
fn lists_the_browsers_by_url_scheme_or_category_in_name_order() {
    let fixture = Fixture::new();
    for program in [
        "firefox",
        "microsoft-edge-stable",
        "google-chrome-stable",
        "gnome-text-editor",
    ] {
        fixture.program(program);
    }
    let system = fixture.system();
    let edge_types = "MimeType=text/html;x-scheme-handler/http;x-scheme-handler/https;\nCategories=Network;WebBrowser;\n";
    fixture.write(
        &system,
        "firefox_firefox.desktop",
        &app(
            "Firefox",
            "firefox %u",
            "MimeType=text/html;x-scheme-handler/https;\n",
        ),
    );
    fixture.write(
        &system,
        "microsoft-edge.desktop",
        &app("Microsoft Edge", "microsoft-edge-stable %U", edge_types),
    );
    fixture.write(
        &system,
        "com.microsoft.Edge.desktop",
        &app(
            "Microsoft Edge",
            "microsoft-edge-stable %U",
            &format!("{edge_types}NoDisplay=true\n"),
        ),
    );
    fixture.write(
        &system,
        "google-chrome.desktop",
        &app(
            "Google Chrome",
            "google-chrome-stable %U",
            "Categories=Network;WebBrowser;\n",
        ),
    );
    fixture.write(
        &system,
        "org.gnome.TextEditor.desktop",
        &app(
            "Text Editor",
            "gnome-text-editor %U",
            "MimeType=text/html;text/plain;\n",
        ),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    let browsers: Vec<&str> = apps
        .browsers()
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(
        browsers,
        [
            "firefox_firefox.desktop",
            "google-chrome.desktop",
            "microsoft-edge.desktop"
        ]
    );
}

#[test]
fn gives_the_command_of_a_browser_with_the_url() {
    let fixture = Fixture::new();
    fixture.program("microsoft-edge-stable");
    fixture.write(
        &fixture.system(),
        "microsoft-edge.desktop",
        &app(
            "Microsoft Edge",
            "microsoft-edge-stable %U",
            "Categories=WebBrowser;\n",
        ),
    );

    let apps = Applications::load(fixture.env(), gnome::is_settings_page);

    let edge = apps.browsers()[0];
    assert_eq!(
        edge.command(Some("https://github.com/LOLm/cat/pulls")),
        ["microsoft-edge-stable", "https://github.com/LOLm/cat/pulls"]
    );
}

#[test]
fn can_move_the_list_to_a_different_thread() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<Applications>();
}
