//! The actions that need the desktop: open a file, a folder or a URL, start
//! an application, run a script, and the system commands.
//!
//! This is the seam for the crate `petlauncher-gnome`: the next task gives
//! `execute` its body with the `Desktop` trait of the core and GIO. Now the
//! function only writes the action to the log.

use petlauncher_core::{Action, DesktopError};

use crate::log;

/// Executes an action for which `model::effect` gives `Effect::Desktop`.
pub fn execute(action: &Action) -> Result<(), DesktopError> {
    log::info(format_args!("desktop action {action:?}"));
    Ok(())
}
