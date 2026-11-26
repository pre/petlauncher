//! The functions that depend on the desktop. `petlauncher-gnome`
//! implements them for GNOME. The core does not call this trait: the program
//! calls it to execute an `Action` and for its windows.

use std::fmt;
use std::path::Path;

use crate::contract::SystemCommand;
use crate::system::SettingsPage;

/// An error of a desktop function, as text for the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopError(pub String);

impl fmt::Display for DesktopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DesktopError {}

/// The light or dark style of the desktop (R17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorScheme {
    #[default]
    Light,
    Dark,
}

/// The functions that depend on the desktop. See the table "Common core and
/// desktop parts" of the requirements.
///
/// The program calls these methods on the thread of the user interface, so
/// the trait does not need `Send`.
pub trait Desktop {
    /// R1, R2: opens the directory in the file manager. For
    /// `Action::OpenFolder`. GNOME: `org.freedesktop.FileManager1.ShowFolders`.
    fn open_folder(&self, folder: &Path) -> Result<(), DesktopError>;

    /// R1, R2: opens the folder that contains `item` in the file manager,
    /// with `item` selected. For `Action::ShowInFolder`. GNOME:
    /// `org.freedesktop.FileManager1.ShowItems`.
    fn show_in_folder(&self, item: &Path) -> Result<(), DesktopError>;

    /// R16: lock, suspend, log out, restart or power off. For
    /// `Action::System`. Log out, restart and power off show the
    /// confirmation window of the desktop.
    fn run_system_command(&self, command: SystemCommand) -> Result<(), DesktopError>;

    /// R16: the pages of the system settings. The program gives them to a
    /// `SettingsPages` provider in `Slot::SettingsPages`.
    fn settings_pages(&self) -> Vec<SettingsPage>;

    /// R4: the global key that runs `petlauncher toggle`, in the GTK
    /// accelerator syntax such as `<Super>space`. `None`: no key is set.
    fn global_key(&self) -> Result<Option<String>, DesktopError>;

    /// R4: sets the global key that runs `command`.
    fn set_global_key(&self, accelerator: &str, command: &str) -> Result<(), DesktopError>;

    /// R11: puts the launcher window at the center of the monitor in use.
    /// The program calls it each time the window shows. On GNOME the
    /// extension does the work, and this method can do nothing.
    fn center_window(&self) -> Result<(), DesktopError>;

    /// R17: the style of the desktop now.
    fn color_scheme(&self) -> ColorScheme;

    /// R17: calls `on_change` on the thread of the user interface each time
    /// the style of the desktop changes.
    fn watch_color_scheme(&self, on_change: Box<dyn Fn(ColorScheme)>);

    /// R3: shows or removes the icon in the top bar.
    fn set_tray_icon_visible(&self, visible: bool) -> Result<(), DesktopError>;
}
