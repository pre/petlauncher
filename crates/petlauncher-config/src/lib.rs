//! Settings and shortcuts of petlauncher.
//!
//! The data types are in `model`. The owner keeps the values in two TOML
//! files in `~/.config/petlauncher/`: `settings.toml` and `shortcuts.toml`.

pub mod model;

pub use model::{
    Browser, Dictionary, FileSearch, KeepQuery, ParseKeepQueryError, Settings, Shortcut,
};
