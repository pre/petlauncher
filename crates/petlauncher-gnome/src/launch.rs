//! The execution of an `Action`. This module has no GNOME code: it uses
//! GIO and the `Desktop` trait only, so the part for a different desktop
//! can use it with no change.

use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use gio::prelude::*;

use petlauncher_core::config::Browser;
use petlauncher_core::{Action, Desktop, DesktopError, FolderOpened};

/// The result of `execute`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Executed {
    /// The action is done. `Action::Nothing` also gives `Done`.
    Done,
    /// The file manager interface failed, so the default application for
    /// folders opened the folder (`FolderOpened::Fallback`). `reason` is for
    /// the log.
    Fallback { reason: String },
    /// The program must do the action itself: `Copy`, `OpenDictionary` and
    /// `SetQuery`.
    NotHandled,
}

impl From<FolderOpened> for Executed {
    fn from(opened: FolderOpened) -> Self {
        match opened {
            FolderOpened::FileManager => Self::Done,
            FolderOpened::Fallback { reason } => Self::Fallback { reason },
        }
    }
}

/// Executes the parts of `action` that need no GTK. The folder and system
/// actions go to `desktop`. `context` is the launch context of the program,
/// for the activation token of a new window.
///
/// Call it while the launcher window is still visible, and hide the window
/// after it, so that the activation token comes from the window that has
/// the focus.
pub fn execute(
    action: &Action,
    desktop: &dyn Desktop,
    context: Option<&gio::AppLaunchContext>,
) -> Result<Executed, DesktopError> {
    let done = |result: Result<(), DesktopError>| result.map(|()| Executed::Done);
    match action {
        Action::Nothing => Ok(Executed::Done),
        Action::OpenFolder(folder) => desktop.open_folder(folder).map(Executed::from),
        Action::ShowInFolder(item) => desktop.show_in_folder(item).map(Executed::from),
        Action::System(command) => done(desktop.run_system_command(*command)),
        Action::OpenFile(path) => done(open_file(path, context)),
        Action::LaunchDesktopEntry(path) => done(launch_desktop_entry(path, context)),
        Action::OpenUrl { url, browser } => done(open_url(url, browser.as_ref(), context)),
        Action::RunScript { script, argument } => {
            done(run_script(script, argument, &glib::home_dir()))
        }
        Action::SetQuery(_) | Action::Copy(_) | Action::OpenDictionary(_) => {
            Ok(Executed::NotHandled)
        }
    }
}

fn open_file(path: &Path, context: Option<&gio::AppLaunchContext>) -> Result<(), DesktopError> {
    let could_not_open =
        |error: glib::Error| DesktopError(format!("Could not open {}: {error}", path.display()));
    let uri = glib::filename_to_uri(path, None).map_err(could_not_open)?;
    gio::AppInfo::launch_default_for_uri(&uri, context).map_err(could_not_open)
}

fn launch_desktop_entry(
    path: &Path,
    context: Option<&gio::AppLaunchContext>,
) -> Result<(), DesktopError> {
    let entry = gio_unix::DesktopAppInfo::from_filename(path).ok_or_else(|| {
        DesktopError(format!(
            "{} is not a desktop entry that can start.",
            path.display()
        ))
    })?;
    entry
        .launch(&[], context)
        .map_err(|error| DesktopError(format!("Could not start {}: {error}", path.display())))
}

/// R12: the default browser, an installed browser, or a command. A command
/// is split with the quoting rules of the shell, and the URL is its last
/// argument. No shell runs it, so `~` and `$NAME` stay as they are.
fn open_url(
    url: &str,
    browser: Option<&Browser>,
    context: Option<&gio::AppLaunchContext>,
) -> Result<(), DesktopError> {
    let could_not_open =
        |error: glib::Error| DesktopError(format!("Could not open {url}: {error}"));
    match browser {
        None => gio::AppInfo::launch_default_for_uri(url, context).map_err(could_not_open),
        Some(Browser::App(id)) => gio_unix::DesktopAppInfo::new(id)
            .ok_or_else(|| DesktopError(format!("The browser {id} is not installed.")))?
            .launch_uris(&[url], context)
            .map_err(could_not_open),
        Some(Browser::Command(command)) => {
            glib::shell_parse_argv(command).map_err(|error| {
                DesktopError(format!(
                    "The browser command \"{command}\" is not correct: {error}"
                ))
            })?;
            // GIO adds " %u", which becomes the URL. A "%" of the command
            // must be "%%" for GIO.
            gio::AppInfo::create_from_commandline(
                command.replace('%', "%%"),
                None,
                gio::AppInfoCreateFlags::SUPPORTS_URIS
                    | gio::AppInfoCreateFlags::SUPPORTS_STARTUP_NOTIFICATION,
            )
            .and_then(|browser| browser.launch_uris(&[url], context))
            .map_err(could_not_open)
        }
    }
}

/// R6: `sh -c script`, with `argument` as `$1`. The script runs in its own
/// process group, so a signal to the launcher, for example Ctrl+C in a
/// terminal, does not stop it, and it continues when the launcher stops. A
/// thread waits for its end, so that it does not stay as a zombie process.
fn run_script(script: &str, argument: &str, working_dir: &Path) -> Result<(), DesktopError> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(script)
        .arg("petlauncher-script")
        .arg(argument)
        .current_dir(working_dir)
        .stdin(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|error| DesktopError(format!("Could not start the script: {error}")))?;
    std::thread::spawn(move || child.wait());
    Ok(())
}
