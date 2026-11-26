use petlauncher_config::Settings;

use crate::cancel::CancelToken;
use crate::contract::Row;

/// A source of rows, for example the calculator or the file index.
///
/// The engine calls `search` on its own thread, not on the thread of the
/// user interface. A provider in the general search (`Slot::Calculator` to
/// `Slot::SettingsPages`) must answer from memory in a few milliseconds,
/// because the engine calls these providers one after the other for each
/// typed character. A slow provider must be in a keyword slot
/// (`Slot::FileSearch`, `Slot::Dictionary`), must read `cancel` in its long
/// loops and must return early when it is cancelled. A provider must not
/// wait for the network.
pub trait Provider: Send + Sync {
    /// Gives the rows for `text`. In the general search `text` is the full
    /// query without the spaces at its start and end. In a keyword mode
    /// `text` is the argument after the keyword. The provider sorts its own
    /// rows, best first.
    fn search(&self, text: &str, settings: &Settings, cancel: &CancelToken) -> Vec<Row>;
}

/// A closure is a provider. Tests and small providers use this.
impl<F> Provider for F
where
    F: Fn(&str, &Settings, &CancelToken) -> Vec<Row> + Send + Sync,
{
    fn search(&self, text: &str, settings: &Settings, cancel: &CancelToken) -> Vec<Row> {
        self(text, settings, cancel)
    }
}

/// The place of a provider in the search. The general search shows the
/// slots from `Calculator` to `SettingsPages` in this order, but in the
/// slots from `Applications` to `SettingsPages` the rows whose title matches
/// the query come first (`docs/core.md`). `FileSearch` and `Dictionary`
/// answer only in their keyword mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Slot {
    /// R8, R15.
    Calculator,
    /// R20.
    Time,
    /// R3 "Include applications in search" switches this slot off.
    Applications,
    /// R16. `Engine::new` adds the built-in `SystemCommands` here.
    SystemCommands,
    /// R16. The program adds `SettingsPages` with `Desktop::settings_pages`.
    SettingsPages,
    /// R14, after the keyword `settings.file_search.keyword`.
    FileSearch,
    /// R19, after the keyword `settings.dictionary.keyword`.
    Dictionary,
}
