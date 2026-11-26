//! The actions that need the desktop: open a file, a folder or a URL, start
//! an application, run a script, and the system commands. `GnomeDesktop`
//! of `petlauncher-gnome` executes them.

use petlauncher_core::{Action, DesktopError};
use petlauncher_gnome::{Executed, GnomeDesktop};

use crate::log;

/// Executes an action for which `model::effect` gives `Effect::Desktop`.
/// Call it while the launcher window is still visible, so that the new
/// window gets the activation token of the window with the focus.
pub fn execute(desktop: &GnomeDesktop, action: &Action) -> Result<(), DesktopError> {
    let executed = desktop.execute(action)?;
    if let Some(line) = log_line(&executed, action) {
        log::info(line);
    }
    Ok(())
}

/// The log line for a result that is a success but not the normal one.
fn log_line(executed: &Executed, action: &Action) -> Option<String> {
    match executed {
        Executed::Done => None,
        Executed::Fallback { reason } => Some(format!(
            "the file manager interface failed, so the default application opened the folder: {reason}"
        )),
        Executed::NotHandled => Some(format!("the desktop did not execute {action:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_normal_result_gives_no_log_line() {
        assert_eq!(log_line(&Executed::Done, &Action::Nothing), None);
    }

    #[test]
    fn a_fallback_and_an_action_of_the_program_go_to_the_log() {
        let fallback = log_line(
            &Executed::Fallback {
                reason: "no FileManager1".into(),
            },
            &Action::OpenFolder("/a".into()),
        )
        .unwrap();
        let not_handled = log_line(&Executed::NotHandled, &Action::Copy("x".into())).unwrap();

        assert!(fallback.contains("no FileManager1"), "{fallback}");
        assert!(not_handled.contains("Copy"), "{not_handled}");
    }
}
