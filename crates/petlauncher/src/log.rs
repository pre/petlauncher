//! Log lines on the standard error. GNOME sends them to the journal.
//!
//! Each line has the monotonic time in milliseconds, the same clock as
//! `GLib.get_monotonic_time()` in GNOME Shell, so that a test can compare
//! the lines with the events of the shell.

use std::fmt::Display;
use std::sync::OnceLock;

pub fn info(message: impl Display) {
    let milliseconds = gtk::glib::monotonic_time() as f64 / 1000.0;
    eprintln!("petlauncher {milliseconds:.1}: {message}");
}

/// A line only when the environment variable `PETLAUNCHER_DEBUG` is set.
pub fn debug(message: impl Display) {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    if *ENABLED.get_or_init(|| std::env::var_os("PETLAUNCHER_DEBUG").is_some()) {
        info(message);
    }
}
