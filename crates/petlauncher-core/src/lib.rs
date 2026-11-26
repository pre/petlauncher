//! The core of petlauncher: the contract with the user interface, the query
//! engine, the path mode, the keys and the kept query.
//!
//! The core does not start a process, open a URL, use the clipboard or call
//! D-Bus. It returns `Action` values, and the program executes them. See
//! `docs/core.md` in the repository.

mod cancel;
mod contract;
mod desktop;
mod engine;
mod kept_query;
mod keys;
mod matching;
mod path_mode;
mod provider;
mod shortcuts;
mod system;

pub use cancel::CancelToken;
pub use contract::{
    Action, Icon, Key, Mode, Outcome, Results, Row, RowKind, SystemCommand, TypedPath,
};
pub use desktop::{ColorScheme, Desktop, DesktopError};
pub use engine::Engine;
pub use kept_query::{keep_query_warning, Clock, CloseReason, KeptQuery, ManualClock, SystemClock};
pub use keys::activate;
pub use matching::match_rank;
pub use petlauncher_config as config;
pub use provider::{Provider, Slot};
pub use system::{SettingsPage, SettingsPages, SystemCommands};
