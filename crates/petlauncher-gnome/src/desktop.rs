//! `GnomeDesktop`: the `Desktop` trait for GNOME.

use std::cell::{OnceCell, RefCell};
use std::path::Path;
use std::time::UNIX_EPOCH;

use gio::prelude::*;
use glib::variant::ToVariant;
use petlauncher_core::config::{CustomPosition, WindowPosition};
use petlauncher_core::{Action, Desktop, DesktopError, FolderOpened, KeptUntil, SystemCommand};

use crate::file_manager;
use crate::launch::{self, Executed};
use crate::session::{self, Bus};
use crate::shortcuts::CustomShortcuts;

/// The name of the action with the boolean state "Show tray icon" (R3). The
/// GNOME Shell extension reads it on D-Bus (ADR 0005).
pub const TRAY_ICON_ACTION: &str = "tray-icon";

/// The name of the action whose state tells the GNOME Shell extension until
/// when the next show of the launcher has the kept query of R9 (ADR 0009).
pub const KEPT_QUERY_ACTION: &str = "kept-query-until";

/// The name of the action whose state gives the GNOME Shell extension the
/// setting `window_position` (ADR 0009).
pub const WINDOW_POSITION_ACTION: &str = "window-position";

/// The GNOME desktop: the session bus, the system bus, the custom shortcuts
/// in GSettings, and the actions that the extension reads. Make it on the
/// thread of the user interface.
pub struct GnomeDesktop {
    session_bus: gio::DBusConnection,
    system_bus: OnceCell<gio::DBusConnection>,
    shortcuts: Result<CustomShortcuts, DesktopError>,
    launch_context: RefCell<Option<gio::AppLaunchContext>>,
    tray_icon: gio::SimpleAction,
    kept_query: gio::SimpleAction,
    window_position: gio::SimpleAction,
}

impl GnomeDesktop {
    /// Connects to the session bus and opens the GNOME custom shortcuts.
    /// When the shortcut schemas are missing, only the methods of R4 fail.
    /// The system bus connects at the first suspend.
    pub fn new() -> Result<Self, DesktopError> {
        let session_bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
            .map_err(|error| {
                DesktopError(format!("Could not connect to the session bus: {error}"))
            })?;
        Ok(Self::with_parts(session_bus, None, CustomShortcuts::new()))
    }

    /// Uses the given connections and custom shortcuts. A test gives
    /// private buses and the memory GSettings backend. With `system_bus`
    /// `None`, the first suspend connects to the system bus.
    pub fn with_parts(
        session_bus: gio::DBusConnection,
        system_bus: Option<gio::DBusConnection>,
        shortcuts: Result<CustomShortcuts, DesktopError>,
    ) -> Self {
        let tray_icon =
            gio::SimpleAction::new_stateful(TRAY_ICON_ACTION, None, &false.to_variant());
        tray_icon.set_enabled(false);
        let kept_query =
            gio::SimpleAction::new_stateful(KEPT_QUERY_ACTION, None, &0i64.to_variant());
        kept_query.set_enabled(false);
        let window_position = gio::SimpleAction::new_stateful(
            WINDOW_POSITION_ACTION,
            None,
            &("center", 0.0, 0.0).to_variant(),
        );
        window_position.set_enabled(false);
        Self {
            session_bus,
            system_bus: system_bus.map(OnceCell::from).unwrap_or_default(),
            shortcuts,
            launch_context: RefCell::new(None),
            tray_icon,
            kept_query,
            window_position,
        }
    }

    /// The launch context for the programs and windows that petlauncher
    /// starts. The GTK program gives `gdk::Display::app_launch_context()`,
    /// so that each new window gets an activation token and the focus.
    pub fn set_launch_context(&self, context: Option<gio::AppLaunchContext>) {
        self.launch_context.replace(context);
    }

    /// R4: the custom shortcuts, for the functions that are not in the
    /// `Desktop` trait: `conflicts`, `set_global_key_if_free` and
    /// `remove_key`.
    pub fn shortcuts(&self) -> Result<&CustomShortcuts, DesktopError> {
        self.shortcuts.as_ref().map_err(Clone::clone)
    }

    /// Adds the actions `tray-icon`, `kept-query-until` and
    /// `window-position` to the application, so that the GNOME Shell
    /// extension can read them. The `set_` methods change their states.
    /// Call it one time, for example in the `startup` signal.
    pub fn add_actions(&self, actions: &impl IsA<gio::ActionMap>) {
        actions.add_action(&self.tray_icon);
        actions.add_action(&self.kept_query);
        actions.add_action(&self.window_position);
    }

    /// The state of `window-position`: the name of the setting and the
    /// place of `custom`, as fractions of the work area. Set it after each
    /// change of the settings.
    pub fn set_window_position(&self, position: WindowPosition, custom: Option<CustomPosition>) {
        let state = match (position, custom) {
            (WindowPosition::Custom, Some(place)) => ("custom", place.x, place.y),
            (WindowPosition::Custom, None) => ("center", 0.0, 0.0),
            (position, _) => (position.name(), 0.0, 0.0),
        };
        self.window_position.set_state(&state.to_variant());
    }

    /// The state of `kept-query-until`: the wall-clock time in microseconds
    /// since 1970, 0 for no kept query, and -1 for no time limit. Set it
    /// after a hide of the launcher, not at a show: D-Bus sends the change
    /// later, and the extension reads the state when the window maps.
    pub fn set_kept_query_until(&self, until: KeptUntil) {
        let micros = match until {
            KeptUntil::Nothing => 0,
            KeptUntil::Forever => -1,
            KeptUntil::Time(time) => time
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| i64::try_from(since.as_micros()).unwrap_or(-1)),
        };
        self.kept_query.set_state(&micros.to_variant());
    }

    /// Executes the parts of `action` that need no GTK. See `execute`.
    pub fn execute(&self, action: &Action) -> Result<Executed, DesktopError> {
        launch::execute(action, self, self.launch_context.borrow().as_ref())
    }

    fn system_bus(&self) -> Result<&gio::DBusConnection, DesktopError> {
        if let Some(bus) = self.system_bus.get() {
            return Ok(bus);
        }
        let bus =
            gio::bus_get_sync(gio::BusType::System, gio::Cancellable::NONE).map_err(|error| {
                DesktopError(format!("Could not connect to the system bus: {error}"))
            })?;
        Ok(self.system_bus.get_or_init(|| bus))
    }
}

impl Desktop for GnomeDesktop {
    fn open_folder(&self, folder: &Path) -> Result<FolderOpened, DesktopError> {
        file_manager::open_folder(
            &self.session_bus,
            folder,
            self.launch_context.borrow().as_ref(),
        )
    }

    fn show_in_folder(&self, item: &Path) -> Result<FolderOpened, DesktopError> {
        file_manager::show_in_folder(
            &self.session_bus,
            item,
            self.launch_context.borrow().as_ref(),
        )
    }

    fn run_system_command(&self, command: SystemCommand) -> Result<(), DesktopError> {
        let bus = match session::bus_of(command) {
            Bus::Session => &self.session_bus,
            Bus::System => self.system_bus()?,
        };
        session::run(bus, command)
    }

    fn global_key(&self) -> Result<Option<String>, DesktopError> {
        Ok(self.shortcuts()?.global_key())
    }

    fn set_global_key(&self, accelerator: &str, command: &str) -> Result<(), DesktopError> {
        self.shortcuts()?.set_global_key(accelerator, command)
    }

    /// The GNOME Shell extension centers the window (ADR 0005).
    fn center_window(&self) -> Result<(), DesktopError> {
        Ok(())
    }

    fn set_tray_icon_visible(&self, visible: bool) -> Result<(), DesktopError> {
        self.tray_icon.set_state(&visible.to_variant());
        Ok(())
    }
}
