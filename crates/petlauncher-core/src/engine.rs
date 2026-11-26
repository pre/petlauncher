//! The query engine: it selects the mode of the query and collects the rows.

use std::path::{Path, PathBuf};

use petlauncher_config::{Settings, Shortcut};

use crate::cancel::CancelToken;
use crate::contract::{Mode, Results, Row};
use crate::matching::match_rank;
use crate::path_mode;
use crate::provider::{Provider, Slot};
use crate::shortcuts;
use crate::system::SystemCommands;

/// The slots of the general search whose rows come first: their rows are
/// answers to the query.
const ANSWER_SLOTS: [Slot; 2] = [Slot::Calculator, Slot::Time];

/// The slots of the general search with named items, in the order of the
/// rows. A row whose title matches the query comes before the rows that
/// match only in other text, for example inside a word or in a keyword.
const NAMED_SLOTS: [Slot; 3] = [
    Slot::Applications,
    Slot::SystemCommands,
    Slot::SettingsPages,
];

/// The query engine. Make one at the start, add the providers, and then
/// share it with `Arc` between the threads. `search` takes `&self`, so many
/// searches can run at the same time.
pub struct Engine {
    home: PathBuf,
    providers: Vec<(Slot, Box<dyn Provider>)>,
}

impl Engine {
    /// Makes an engine with the built-in `SystemCommands` provider. `home`
    /// is the home directory, which `~` means in a typed path.
    pub fn new(home: impl Into<PathBuf>) -> Self {
        let mut engine = Self {
            home: path_mode::normalize(&home.into()),
            providers: Vec::new(),
        };
        engine.register(Slot::SystemCommands, SystemCommands);
        engine
    }

    /// Adds a provider to a slot. The rows of the providers in one slot come
    /// in the order of `register`.
    pub fn register(&mut self, slot: Slot, provider: impl Provider + 'static) -> &mut Self {
        self.providers.push((slot, Box::new(provider)));
        self
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Gives the results for the query.
    ///
    /// Call it on a worker thread, not on the thread of the user interface.
    /// When the query changes, cancel the token of the old search. The
    /// engine then stops before the next provider, and gives the rows that
    /// it has. Do not show the results of a cancelled search.
    pub fn search(
        &self,
        query: &str,
        settings: &Settings,
        shortcuts: &[Shortcut],
        cancel: &CancelToken,
    ) -> Results {
        let text = query.trim();
        if text.is_empty() {
            return results(query, Mode::Empty, Vec::new());
        }
        if let Some(query_path) = path_mode::parse(query, &self.home) {
            return path_mode::search(query, &query_path, &self.home, cancel);
        }
        if let Some((keyword, argument)) = split_keyword(text) {
            if shortcuts::same_keyword(keyword, &settings.file_search.keyword) {
                let rows = self.slot_rows(Slot::FileSearch, argument, settings, cancel);
                return results(query, Mode::FileSearch, rows);
            }
            if shortcuts::same_keyword(keyword, &settings.dictionary.keyword) {
                let rows = self.slot_rows(Slot::Dictionary, argument, settings, cancel);
                return results(query, Mode::Dictionary, rows);
            }
            let rows = shortcuts::keyword_rows(shortcuts, keyword, argument, &self.home);
            if !rows.is_empty() {
                return results(query, Mode::Shortcut, rows);
            }
        }
        let rows = self.general_rows(text, settings, shortcuts, cancel);
        results(query, Mode::General, rows)
    }

    fn general_rows(
        &self,
        text: &str,
        settings: &Settings,
        shortcuts: &[Shortcut],
        cancel: &CancelToken,
    ) -> Vec<Row> {
        let mut rows = shortcuts::keyword_rows(shortcuts, text, "", &self.home);
        for slot in ANSWER_SLOTS {
            rows.extend(self.slot_rows(slot, text, settings, cancel));
        }
        let mut named = Vec::new();
        for slot in NAMED_SLOTS {
            if slot == Slot::Applications && !settings.include_applications {
                continue;
            }
            named.extend(self.slot_rows(slot, text, settings, cancel));
        }
        let (title_matches, other_matches): (Vec<Row>, Vec<Row>) = named
            .into_iter()
            .partition(|row| match_rank(text, &row.title, &NO_KEYWORDS).is_some());
        rows.extend(title_matches);
        rows.extend(other_matches);
        if rows.is_empty() && !cancel.is_cancelled() {
            rows = shortcuts::fallback_rows(shortcuts, text, &self.home);
        }
        rows
    }

    fn slot_rows(
        &self,
        slot: Slot,
        text: &str,
        settings: &Settings,
        cancel: &CancelToken,
    ) -> Vec<Row> {
        let mut rows = Vec::new();
        for (_, provider) in self.providers.iter().filter(|(s, _)| *s == slot) {
            if cancel.is_cancelled() {
                break;
            }
            rows.extend(provider.search(text, settings, cancel));
        }
        rows
    }
}

/// Splits `keyword argument` at the first space. Gives `None` when the text
/// has no space. The argument has no spaces at its start and end.
fn split_keyword(text: &str) -> Option<(&str, &str)> {
    let end = text.find(char::is_whitespace)?;
    Some((&text[..end], text[end..].trim()))
}

const NO_KEYWORDS: [&str; 0] = [];

fn results(query: &str, mode: Mode, rows: Vec<Row>) -> Results {
    Results {
        query: query.to_owned(),
        mode,
        selected: (!rows.is_empty()).then_some(0),
        rows,
        typed_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keyword_is_the_text_before_the_first_space() {
        assert_eq!(split_keyword("g rust  traits"), Some(("g", "rust  traits")));
        assert_eq!(split_keyword("rektangel\t123"), Some(("rektangel", "123")));
        assert_eq!(split_keyword("rektangel"), None);
    }
}
