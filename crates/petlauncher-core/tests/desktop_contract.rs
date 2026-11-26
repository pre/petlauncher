//! The `Desktop` trait can be implemented and used as a trait object.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use petlauncher_core::{
    Action, Clock, Desktop, DesktopError, FolderOpened, SystemClock, SystemCommand,
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
    fn open_folder(&self, folder: &Path) -> Result<FolderOpened, DesktopError> {
        self.record(format!("open_folder {}", folder.display()))?;
        Ok(FolderOpened::FileManager)
    }

    fn show_in_folder(&self, item: &Path) -> Result<FolderOpened, DesktopError> {
        self.record(format!("show_in_folder {}", item.display()))?;
        Ok(FolderOpened::Fallback {
            reason: "no file manager".into(),
        })
    }

    fn run_system_command(&self, command: SystemCommand) -> Result<(), DesktopError> {
        self.record(format!("system {}", command.title()))
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

    fn set_tray_icon_visible(&self, _visible: bool) -> Result<(), DesktopError> {
        Ok(())
    }
}

/// The part of the program that gives an action to the desktop.
fn execute(desktop: &dyn Desktop, action: &Action) -> Result<(), DesktopError> {
    match action {
        Action::OpenFolder(folder) => desktop.open_folder(folder).map(drop),
        Action::ShowInFolder(item) => desktop.show_in_folder(item).map(drop),
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
fn the_file_manager_methods_tell_if_the_desktop_used_a_fallback() {
    let desktop = FakeDesktop::default();

    let opened = desktop.open_folder(Path::new("/a")).unwrap();
    let shown = desktop.show_in_folder(Path::new("/a/b.png")).unwrap();

    assert_eq!(opened, FolderOpened::FileManager);
    assert_eq!(
        shown,
        FolderOpened::Fallback {
            reason: "no file manager".into()
        }
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
