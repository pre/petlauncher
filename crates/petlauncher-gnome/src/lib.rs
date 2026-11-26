//! The GNOME part of petlauncher: the `Desktop` trait of `petlauncher-core`
//! for GNOME, and the execution of an `Action`. See `docs/gnome-desktop.md`
//! in the repository.

mod accelerator;
mod bindings;
mod desktop;
mod file_manager;
mod launch;
mod session;
mod shortcuts;

pub use accelerator::{validate_accelerator, AcceleratorError};
pub use bindings::{BindingKind, Conflict, CUSTOM_SECTION, SHORTCUTS_PAGE};
pub use desktop::{GnomeDesktop, KEPT_QUERY_ACTION, TRAY_ICON_ACTION};
pub use launch::{execute, Executed};
/// R16: the rule that finds the pages of GNOME Settings. Give it to
/// `petlauncher_apps::Applications::load`.
pub use petlauncher_apps::gnome::is_settings_page;
pub use shortcuts::{
    CustomShortcut, CustomShortcuts, SHORTCUT_NAME, SHORTCUT_PATH, TOGGLE_COMMAND,
};
