//! "Run in background" off (R3): the process stops when no window of the
//! program shows.

use std::time::Duration;

use adw::prelude::*;
use gtk::glib;

use crate::log;

/// The time from the hide of a window to the stop.
pub const STOP_DELAY: Duration = Duration::from_secs(1);

/// Call it when a window of the program hides and "Run in background" is
/// off. After `STOP_DELAY` the process stops, but only when no window of
/// `app` shows then: the launcher, the dictionary window and the settings
/// window each keep the process. The delay has a second reason: on Wayland
/// the text of a Copy belongs to this process, and GNOME Shell keeps a copy
/// of the clipboard when its owner stops, but only after it read the text.
/// A test showed the previous clipboard text after a stop at once.
pub fn stop_when_no_window_shows(app: &impl IsA<gtk::Application>) {
    let app = app.clone().upcast::<gtk::Application>();
    glib::timeout_add_local_once(STOP_DELAY, move || {
        if app.windows().iter().any(|window| window.is_visible()) {
            return;
        }
        log::info("\"Run in background\" is off and no window shows: stop");
        app.quit();
    });
}
