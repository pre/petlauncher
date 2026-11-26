//! R16: the system commands through GNOME Shell, the GNOME session manager
//! and logind. The calls are the calls that the system menu of GNOME Shell
//! 46 makes, so log out, restart and power off show the usual confirmation
//! window. `docs/gnome-desktop.md` gives the evidence.

use glib::variant::ToVariant;
use petlauncher_core::{DesktopError, SystemCommand};

/// The bus of a system command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bus {
    Session,
    System,
}

/// Lock waits until GNOME Shell shows the lock screen. Suspend can wait for
/// a password dialog of polkit.
const TIMEOUT_MS: i32 = 10_000;

/// `Logout` mode 0: normal, with the confirmation window.
const LOGOUT_NORMAL: u32 = 0;

struct Call {
    bus: Bus,
    name: &'static str,
    path: &'static str,
    interface: &'static str,
    method: &'static str,
    arguments: Option<glib::Variant>,
}

fn call(command: SystemCommand) -> Call {
    let session_manager = |method, arguments| Call {
        bus: Bus::Session,
        name: "org.gnome.SessionManager",
        path: "/org/gnome/SessionManager",
        interface: "org.gnome.SessionManager",
        method,
        arguments,
    };
    match command {
        SystemCommand::Lock => Call {
            bus: Bus::Session,
            name: "org.gnome.ScreenSaver",
            path: "/org/gnome/ScreenSaver",
            interface: "org.gnome.ScreenSaver",
            method: "Lock",
            arguments: None,
        },
        SystemCommand::Suspend => Call {
            bus: Bus::System,
            name: "org.freedesktop.login1",
            path: "/org/freedesktop/login1",
            interface: "org.freedesktop.login1.Manager",
            method: "Suspend",
            // interactive: polkit can ask for a password.
            arguments: Some((true,).to_variant()),
        },
        SystemCommand::LogOut => session_manager("Logout", Some((LOGOUT_NORMAL,).to_variant())),
        SystemCommand::Restart => session_manager("Reboot", None),
        SystemCommand::PowerOff => session_manager("Shutdown", None),
    }
}

pub(crate) fn bus_of(command: SystemCommand) -> Bus {
    call(command).bus
}

pub(crate) fn run(
    connection: &gio::DBusConnection,
    command: SystemCommand,
) -> Result<(), DesktopError> {
    let call = call(command);
    connection
        .call_sync(
            Some(call.name),
            call.path,
            call.interface,
            call.method,
            call.arguments.as_ref(),
            None,
            gio::DBusCallFlags::NONE,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
        )
        .map(drop)
        .map_err(|error| DesktopError(format!("{} failed: {error}", command.title())))
}
