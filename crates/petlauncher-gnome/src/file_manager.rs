//! R1, R2: the file manager through `org.freedesktop.FileManager1`, which
//! Nautilus gives on GNOME. When the call fails, the default application
//! for folders opens the folder.

use std::path::Path;

use gio::prelude::*;
use petlauncher_core::{DesktopError, FolderOpened};

const NAME: &str = "org.freedesktop.FileManager1";
const PATH: &str = "/org/freedesktop/FileManager1";
const INTERFACE: &str = "org.freedesktop.FileManager1";
/// Long enough for the bus to start Nautilus when it does not run.
const TIMEOUT_MS: i32 = 10_000;

pub(crate) fn open_folder(
    bus: &gio::DBusConnection,
    folder: &Path,
    context: Option<&gio::AppLaunchContext>,
) -> Result<FolderOpened, DesktopError> {
    let uri = file_uri(folder)?;
    show(bus, "ShowFolders", &uri, &uri, context)
}

/// The fallback opens the folder that contains `item`, with no selection.
pub(crate) fn show_in_folder(
    bus: &gio::DBusConnection,
    item: &Path,
    context: Option<&gio::AppLaunchContext>,
) -> Result<FolderOpened, DesktopError> {
    let uri = file_uri(item)?;
    let folder_uri = file_uri(item.parent().unwrap_or(item))?;
    show(bus, "ShowItems", &uri, &folder_uri, context)
}

/// `file://` and the path with each byte percent-encoded that a URI path
/// cannot have, for example a space, `#`, `%` and each byte of a letter
/// that is not ASCII.
fn file_uri(path: &Path) -> Result<String, DesktopError> {
    glib::filename_to_uri(path, None)
        .map(String::from)
        .map_err(|error| DesktopError(format!("{}: {error}", path.display())))
}

fn show(
    bus: &gio::DBusConnection,
    method: &str,
    uri: &str,
    folder_uri: &str,
    context: Option<&gio::AppLaunchContext>,
) -> Result<FolderOpened, DesktopError> {
    let startup_id = startup_id(context, folder_uri);
    let called = bus.call_sync(
        Some(NAME),
        PATH,
        INTERFACE,
        method,
        Some(&(vec![uri], startup_id.as_str()).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        TIMEOUT_MS,
        gio::Cancellable::NONE,
    );
    let error = match called {
        Ok(_) => return Ok(FolderOpened::FileManager),
        Err(error) => error,
    };
    if let (Some(context), false) = (context, startup_id.is_empty()) {
        context.launch_failed(&startup_id);
    }
    match gio::AppInfo::launch_default_for_uri(folder_uri, context) {
        Ok(()) => Ok(FolderOpened::Fallback {
            reason: format!("{INTERFACE}.{method}: {error}"),
        }),
        Err(fallback_error) => Err(DesktopError(format!(
            "Could not open {folder_uri}. {INTERFACE}.{method}: {error}. \
             The default application for folders: {fallback_error}"
        ))),
    }
}

/// The activation token that lets the file manager window take the focus
/// on Wayland. Empty when the program gave no launch context.
fn startup_id(context: Option<&gio::AppLaunchContext>, folder_uri: &str) -> String {
    let Some(context) = context else {
        return String::new();
    };
    let Some(file_manager) = gio::AppInfo::default_for_type("inode/directory", false) else {
        return String::new();
    };
    context
        .startup_notify_id(Some(&file_manager), &[gio::File::for_uri(folder_uri)])
        .map(String::from)
        .unwrap_or_default()
}
