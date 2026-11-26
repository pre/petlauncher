//! The providers for the search crates of petlauncher, and the [`Launcher`]
//! object that the program holds.
//!
//! The `Launcher` owns the settings, the query engine with all providers,
//! the kept query, the usage counts and the background threads. The program
//! calls [`Launcher::search`] on a worker thread and the other methods on the
//! thread of the user interface. `docs/launcher.md` in the repository gives
//! the surface, the threads and the background rules.

mod apps;
pub mod autostart;
mod background;
mod calc;
mod dictionary;
mod files;
mod launcher;
mod rates;
mod sync;
mod time;

pub use autostart::Autostart;
pub use launcher::{BrowserStart, Dirs, FetchRates, Hooks, Launcher, OnChange, Update};
pub use petlauncher_apps::{DesktopEntry, Environment, SettingsPageRule};
pub use petlauncher_dict::Error as DictionaryError;
pub use petlauncher_dict::{Entry as DictionaryEntry, Meaning, WordClass};
pub use petlauncher_index::ScopeError;
