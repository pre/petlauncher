//! The functions that depend on the desktop. `petlauncher-gnome`
//! implements them for GNOME. The core does not call this trait: the program
//! calls it to execute an `Action` and for its windows.

use std::fmt;
use std::path::Path;

use crate::contract::SystemCommand;

/// An error of a desktop function, as text for the owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopError(pub String);

impl fmt::Display for DesktopError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DesktopError {}

/// How `Desktop::open_folder` or `Desktop::show_in_folder` opened the
/// folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderOpened {
    /// The file manager opened the folder, and for `show_in_folder` it
    /// selected the item.
    FileManager,
    /// The file manager interface failed, so the default application for
    /// folders opened the folder. No item is selected. `reason` is the error
    /// of the file manager interface, for the log.
    Fallback { reason: String },
}

/// The functions that depend on the desktop. See the table "Common core and
/// desktop parts" of the requirements.
///
/// The program calls these methods on the thread of the user interface, so
/// the trait does not need `Send`.
pub trait Desktop {
    /// R1, R2: opens the directory in the file manager. For
    /// `Action::OpenFolder`. GNOME: `org.freedesktop.FileManager1.ShowFolders`.
    fn open_folder(&self, folder: &Path) -> Result<FolderOpened, DesktopError>;

    /// R1, R2: opens the folder that contains `item` in the file manager,
    /// with `item` selected. For `Action::ShowInFolder`. GNOME:
    /// `org.freedesktop.FileManager1.ShowItems`.
    fn show_in_folder(&self, item: &Path) -> Result<FolderOpened, DesktopError>;

    /// R16: lock, suspend, log out, restart or power off. For
    /// `Action::System`. Log out, restart and power off show the
    /// confirmation window of the desktop.
    fn run_system_command(&self, command: SystemCommand) -> Result<(), DesktopError>;

    /// R4: the global key that runs `petlauncher toggle`, in the GTK
    /// accelerator syntax such as `<Super>space`. `None`: no key is set.
    fn global_key(&self) -> Result<Option<String>, DesktopError>;

    /// R4: sets the global key that runs `command`. An empty `accelerator`
    /// removes the key.
    fn set_global_key(&self, accelerator: &str, command: &str) -> Result<(), DesktopError>;

    /// R11: puts the launcher window at the center of the monitor in use.
    /// The program calls it each time the window shows. On GNOME the
    /// extension does the work, and this method does nothing.
    fn center_window(&self) -> Result<(), DesktopError>;

    /// R3: shows or removes the icon in the top bar.
    fn set_tray_icon_visible(&self, visible: bool) -> Result<(), DesktopError>;
}
