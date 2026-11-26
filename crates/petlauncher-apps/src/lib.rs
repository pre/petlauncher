//! Desktop entries of the installed applications and settings pages, and a
//! ranked search over them.
//!
//! The rules for directories, filters, settings pages and rank are in
//! `docs/applications.md`.

mod catalog;
mod entry;
mod environment;
mod exec;
pub mod gnome;
mod keyfile;
mod locale;
mod search;
mod usage;

pub use catalog::{Applications, Hit, LoadError, SettingsPageRule};
pub use entry::{DesktopEntry, EntryKind};
pub use environment::Environment;
pub use usage::{UsageCounts, UsageStore};
