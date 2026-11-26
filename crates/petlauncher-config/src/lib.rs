//! Settings and shortcuts of petlauncher.
//!
//! The data types are in `model`. The owner keeps the values in two TOML
//! files in `~/.config/petlauncher/`: `settings.toml` and `shortcuts.toml`.
//!
//! The program uses a [`ConfigStore`]. The store reads the two files, keeps
//! the last good values when a file has an error, and writes the changes
//! of the settings window. The changes keep the owner's comments and
//! layout.

pub mod edit;
pub mod error;
pub mod load;
pub mod model;
pub mod paths;
pub mod store;
pub mod validate;
pub mod watch;
mod write;

pub use edit::{SettingsEditor, ShortcutsEditor};
pub use error::{ConfigError, EditError};
pub use load::{load_settings, load_shortcuts};
pub use model::{
    Browser, Dictionary, FileSearch, KeepQuery, ParseKeepQueryError, Settings, Shortcut,
};
pub use paths::{config_dir, ulauncher_shortcuts_file, ConfigFile};
pub use store::{ConfigStore, Reload};
pub use validate::{validate, Problem, ProblemKind, Severity};
pub use watch::ConfigWatcher;
