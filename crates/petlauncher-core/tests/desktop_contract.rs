//! The `Desktop` trait can be implemented and used as a trait object.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use petlauncher_core::{
    Action, Clock, ColorScheme, Desktop, DesktopError, SettingsPage, SystemClock, SystemCommand,
};

#[derive(Default)]
struct FakeDesktop {
    calls: RefCell<Vec<String>>,
}

impl FakeDesktop {
    fn record(&self, call: String) -> Result<(), DesktopError> {
        self.calls.borrow_mut().push(call);
        Ok(())
    }
}

impl Desktop for FakeDesktop {
    fn open_folder(&self, folder: &Path) -> Result<(), DesktopError> {
        self.record(format!("open_folder {}", folder.display()))
    }

    fn show_in_folder(&self, item: &Path) -> Result<(), DesktopError> {
        self.record(format!("show_in_folder {}", item.display()))
    }

    fn run_system_command(&self, command: SystemCommand) -> Result<(), DesktopError> {
        self.record(format!("system {}", command.title()))
    }

    fn settings_pages(&self) -> Vec<SettingsPage> {
        Vec::new()
    }

    fn global_key(&self) -> Result<Option<String>, DesktopError> {
        Err(DesktopError("no custom shortcut".into()))
    }

    fn set_global_key(&self, accelerator: &str, command: &str) -> Result<(), DesktopError> {
        self.record(format!("global_key {accelerator} {command}"))
    }

    fn center_window(&self) -> Result<(), DesktopError> {
        Ok(())
    }

    fn color_scheme(&self) -> ColorScheme {
        ColorScheme::Dark
    }

    fn watch_color_scheme(&self, on_change: Box<dyn Fn(ColorScheme)>) {
        on_change(ColorScheme::Light);
    }

    fn set_tray_icon_visible(&self, _visible: bool) -> Result<(), DesktopError> {
        Ok(())
    }
}

/// The part of the program that gives an action to the desktop.
fn execute(desktop: &dyn Desktop, action: &Action) -> Result<(), DesktopError> {
    match action {
        Action::OpenFolder(folder) => desktop.open_folder(folder),
        Action::ShowInFolder(item) => desktop.show_in_folder(item),
        Action::System(command) => desktop.run_system_command(*command),
        _ => Ok(()),
    }
}

#[test]
fn the_program_gives_the_file_manager_and_system_actions_to_the_desktop() {
    let desktop = FakeDesktop::default();

    for action in [
        Action::OpenFolder(PathBuf::from("/a")),
        Action::ShowInFolder(PathBuf::from("/a/b.png")),
        Action::System(SystemCommand::Lock),
    ] {
        execute(&desktop, &action).unwrap();
    }

    assert_eq!(
        *desktop.calls.borrow(),
        ["open_folder /a", "show_in_folder /a/b.png", "system Lock"]
    );
}

#[test]
fn a_desktop_error_shows_its_text() {
    let desktop: Box<dyn Desktop> = Box::new(FakeDesktop::default());

    let error = desktop.global_key().unwrap_err();

    assert_eq!(error.to_string(), "no custom shortcut");
}

#[test]
fn the_system_clock_gives_the_time_now() {
    let before = SystemTime::now();

    let now = SystemClock.now();

    assert!(now >= before);
}
