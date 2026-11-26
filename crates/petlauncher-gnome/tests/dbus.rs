//! R1, R2, R3, R16: the D-Bus calls of `GnomeDesktop`, against mock
//! services on private buses. No call reaches the session of the owner.

mod support;

use std::path::Path;

use gio::prelude::*;
use petlauncher_core::{Action, Desktop, DesktopError, FolderOpened, SystemCommand};
use petlauncher_gnome::{Executed, GnomeDesktop, TRAY_ICON_ACTION};
use support::{MockService, PrivateBus, Reply};

fn desktop(session: &PrivateBus, system: &PrivateBus) -> GnomeDesktop {
    GnomeDesktop::with_parts(
        session.connect(),
        Some(system.connect()),
        Err(DesktopError("no custom shortcuts in this test".into())),
    )
}

#[test]
fn open_folder_calls_show_folders_with_the_file_uri_and_no_startup_id() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let file_manager = support::file_manager(&session, Reply::Empty);

    let opened = desktop(&session, &system)
        .open_folder(Path::new("/home/owner/lolcat-output/L0L3-IMAGE"))
        .unwrap();

    assert_eq!(opened, FolderOpened::FileManager);
    assert_eq!(
        file_manager.calls(),
        ["ShowFolders (ass) (['file:///home/owner/lolcat-output/L0L3-IMAGE'], '')"]
    );
}

#[test]
fn show_in_folder_calls_show_items_with_the_uri_of_the_item() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let file_manager = support::file_manager(&session, Reply::Empty);

    let opened = desktop(&session, &system)
        .show_in_folder(Path::new(
            "/usr/share/applications/org.gnome.Nautilus.desktop",
        ))
        .unwrap();

    assert_eq!(opened, FolderOpened::FileManager);
    assert_eq!(
        file_manager.calls(),
        ["ShowItems (ass) (['file:///usr/share/applications/org.gnome.Nautilus.desktop'], '')"]
    );
}

#[test]
fn the_uri_encodes_spaces_hash_percent_and_letters_that_are_not_ascii() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let file_manager = support::file_manager(&session, Reply::Empty);

    desktop(&session, &system)
        .show_in_folder(Path::new("/home/owner/a b/#2/100%/Päivä.png"))
        .unwrap();

    assert_eq!(
        file_manager.calls(),
        ["ShowItems (ass) (['file:///home/owner/a%20b/%232/100%25/P%C3%A4iv%C3%A4.png'], '')"]
    );
}

#[test]
fn a_relative_path_is_an_error_and_calls_nothing() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let file_manager = support::file_manager(&session, Reply::Empty);

    let error = desktop(&session, &system)
        .open_folder(Path::new("relative/dir"))
        .unwrap_err();

    assert!(error.0.contains("relative/dir"), "{error}");
    assert_eq!(file_manager.calls(), Vec::<String>::new());
}

#[test]
fn execute_gives_open_folder_and_show_in_folder_to_the_file_manager() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let file_manager = support::file_manager(&session, Reply::Empty);
    let desktop = desktop(&session, &system);

    let opened = desktop
        .execute(&Action::OpenFolder("/srv/a".into()))
        .unwrap();
    let shown = desktop
        .execute(&Action::ShowInFolder("/srv/a/b".into()))
        .unwrap();

    assert_eq!((opened, shown), (Executed::Done, Executed::Done));
    assert_eq!(
        file_manager.calls(),
        [
            "ShowFolders (ass) (['file:///srv/a'], '')",
            "ShowItems (ass) (['file:///srv/a/b'], '')"
        ]
    );
}

fn system_mocks(session: &PrivateBus, system: &PrivateBus) -> [MockService; 3] {
    [
        MockService::start(
            session,
            "org.gnome.ScreenSaver",
            "/org/gnome/ScreenSaver",
            support::SCREEN_SAVER_XML,
            Reply::Empty,
        ),
        MockService::start(
            session,
            "org.gnome.SessionManager",
            "/org/gnome/SessionManager",
            support::SESSION_MANAGER_XML,
            Reply::Empty,
        ),
        MockService::start(
            system,
            "org.freedesktop.login1",
            "/org/freedesktop/login1",
            support::LOGIN1_XML,
            Reply::Empty,
        ),
    ]
}

#[test]
fn lock_calls_the_screen_saver_of_gnome_shell() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let [screen_saver, session_manager, login] = system_mocks(&session, &system);

    desktop(&session, &system)
        .run_system_command(SystemCommand::Lock)
        .unwrap();

    assert_eq!(screen_saver.calls(), ["Lock () ()"]);
    assert_eq!(session_manager.calls().len() + login.calls().len(), 0);
}

#[test]
fn suspend_calls_logind_on_the_system_bus_and_permits_a_password_dialog() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let [screen_saver, session_manager, login] = system_mocks(&session, &system);

    desktop(&session, &system)
        .run_system_command(SystemCommand::Suspend)
        .unwrap();

    assert_eq!(login.calls(), ["Suspend (b) (true,)"]);
    assert_eq!(
        screen_saver.calls().len() + session_manager.calls().len(),
        0
    );
}

#[test]
fn log_out_asks_the_session_manager_for_its_confirmation_window() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let [_, session_manager, _] = system_mocks(&session, &system);

    desktop(&session, &system)
        .run_system_command(SystemCommand::LogOut)
        .unwrap();

    assert_eq!(session_manager.calls(), ["Logout (u) (0,)"]);
}

#[test]
fn restart_and_power_off_call_reboot_and_shutdown_of_the_session_manager() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let [_, session_manager, login] = system_mocks(&session, &system);
    let desktop = desktop(&session, &system);

    desktop.run_system_command(SystemCommand::Restart).unwrap();
    desktop.run_system_command(SystemCommand::PowerOff).unwrap();

    assert_eq!(session_manager.calls(), ["Reboot () ()", "Shutdown () ()"]);
    assert_eq!(login.calls().len(), 0);
}

#[test]
fn execute_gives_a_system_action_to_the_desktop() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let [screen_saver, _, _] = system_mocks(&session, &system);

    let executed = desktop(&session, &system)
        .execute(&Action::System(SystemCommand::Lock))
        .unwrap();

    assert_eq!(executed, Executed::Done);
    assert_eq!(screen_saver.calls(), ["Lock () ()"]);
}

#[test]
fn a_system_command_with_no_service_is_an_error_with_the_command_and_the_cause() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());

    let error = desktop(&session, &system)
        .run_system_command(SystemCommand::Lock)
        .unwrap_err();

    assert!(error.0.starts_with("Lock failed: "), "{error}");
    assert!(error.0.contains("org.gnome.ScreenSaver"), "{error}");
}

#[test]
fn a_system_command_that_the_service_refuses_is_an_error() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let _login = MockService::start(
        &system,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        support::LOGIN1_XML,
        Reply::Error,
    );

    let error = desktop(&session, &system)
        .run_system_command(SystemCommand::Suspend)
        .unwrap_err();

    assert!(error.0.starts_with("Suspend failed: "), "{error}");
    assert!(error.0.contains("fails on purpose"), "{error}");
}

#[test]
fn center_window_does_nothing_because_the_extension_centers_the_window() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());

    assert_eq!(desktop(&session, &system).center_window(), Ok(()));
}

#[test]
fn the_tray_icon_action_is_disabled_and_follows_set_tray_icon_visible() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let desktop = desktop(&session, &system);
    let actions = gio::SimpleActionGroup::new();
    desktop.add_actions(&actions);

    let before = actions.action_state(TRAY_ICON_ACTION);
    desktop.set_tray_icon_visible(true).unwrap();
    let after = actions.action_state(TRAY_ICON_ACTION);

    assert_eq!(before, Some(false.to_variant()));
    assert_eq!(after, Some(true.to_variant()));
    assert!(!actions.is_action_enabled(TRAY_ICON_ACTION));
}

/// The extension reads the state with `org.gtk.Actions` (ADR 0005).
#[test]
fn the_extension_reads_the_tray_icon_state_on_d_bus() {
    let (session, system) = (PrivateBus::start(), PrivateBus::start());
    let desktop = desktop(&session, &system);
    let actions = gio::SimpleActionGroup::new();
    desktop.add_actions(&actions);
    desktop.set_tray_icon_visible(true).unwrap();
    let program = session.connect();
    let extension = session.connect();
    // The exported group answers on the main context of this thread, so the
    // call is asynchronous and this thread runs the context.
    let context = glib::MainContext::new();

    let reply = context
        .with_thread_default(|| {
            program
                .export_action_group("/pet/repo/Petlauncher", &actions)
                .unwrap();
            context.block_on(extension.call_future(
                program.unique_name().as_deref(),
                "/pet/repo/Petlauncher",
                "org.gtk.Actions",
                "Describe",
                Some(&(TRAY_ICON_ACTION,).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                5000,
            ))
        })
        .unwrap()
        .unwrap();

    assert_eq!(reply.print(false), "((false, '', [<true>]),)");
}
