//! The rows of the dictionary query `d <text>` (R19), and the open
//! dictionary that the rows and the dictionary window share.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use petlauncher_core::config::Settings;
use petlauncher_core::{Action, CancelToken, Icon, Provider, Row, RowKind};
use petlauncher_dict::{Dictionary, Entry, Error, Meaning};

use crate::sync::lock;

/// The command that installs the WordNet data on Ubuntu.
pub(crate) const INSTALL_COMMAND: &str = "sudo apt install wordnet-base";

/// The largest number of meaning rows.
const MEANING_ROWS: usize = 20;

/// The largest number of suggestion rows.
const SUGGESTION_ROWS: usize = 10;

/// The WordNet directory of the settings and the dictionary of it. The
/// dictionary opens at the first use, on the thread of that use. An open
/// takes about 50 ms with the real data.
pub(crate) struct DictionaryState {
    home: PathBuf,
    inner: Mutex<Inner>,
}

struct Inner {
    path: PathBuf,
    state: State,
}

enum State {
    Closed,
    Open(Arc<Dictionary>),
    Failed(Arc<Error>),
}

impl DictionaryState {
    /// `setting` is `dictionary.wordnet_path`. `~` in it is `home`.
    pub(crate) fn new(home: &Path, setting: &str) -> Self {
        Self {
            home: home.to_owned(),
            inner: Mutex::new(Inner {
                path: expand_home(setting, home),
                state: State::Closed,
            }),
        }
    }

    /// Uses a new WordNet directory. The next use opens it.
    pub(crate) fn set_path(&self, setting: &str) {
        let path = expand_home(setting, &self.home);
        let mut inner = lock(&self.inner);
        if inner.path != path {
            inner.path = path;
            inner.state = State::Closed;
        }
    }

    /// The next use tries the open again after a failure, for example after
    /// the owner installed the data.
    pub(crate) fn forget_failure(&self) {
        let mut inner = lock(&self.inner);
        if matches!(inner.state, State::Failed(_)) {
            inner.state = State::Closed;
        }
    }

    /// The open dictionary. The first call opens it. A failure stays until
    /// `forget_failure` or `set_path`. Other callers wait for an open that
    /// runs.
    pub(crate) fn get(&self) -> Result<Arc<Dictionary>, Arc<Error>> {
        let mut inner = lock(&self.inner);
        if let State::Closed = inner.state {
            inner.state = match Dictionary::open(&inner.path) {
                Ok(dictionary) => State::Open(Arc::new(dictionary)),
                Err(error) => State::Failed(Arc::new(error)),
            };
        }
        match &inner.state {
            State::Open(dictionary) => Ok(Arc::clone(dictionary)),
            State::Failed(error) => Err(Arc::clone(error)),
            State::Closed => unreachable!("the open above sets the state"),
        }
    }

    pub(crate) fn lookup(&self, word: &str) -> Result<Vec<Entry>, Arc<Error>> {
        self.get()?.lookup(word).map_err(Arc::new)
    }

    pub(crate) fn suggestions(&self, text: &str, limit: usize) -> Result<Vec<String>, Arc<Error>> {
        Ok(self.get()?.suggestions(text, limit))
    }
}

/// The meanings of the typed word first, each with its word class and
/// synonyms. Then the words that start with the typed text, without the
/// typed word. Enter on each row opens the dictionary window for its word.
pub(crate) struct DictionaryProvider(pub Arc<DictionaryState>);

impl Provider for DictionaryProvider {
    fn search(&self, text: &str, _settings: &Settings, cancel: &CancelToken) -> Vec<Row> {
        let dictionary = match self.0.get() {
            Ok(dictionary) => dictionary,
            Err(error) => return vec![error_row(&error)],
        };
        let entries = match dictionary.lookup(text) {
            Ok(entries) => entries,
            Err(error) => return vec![error_row(&error)],
        };
        let mut rows: Vec<Row> = entries
            .iter()
            .flat_map(|entry| {
                entry
                    .meanings
                    .iter()
                    .map(move |meaning| meaning_row(entry, meaning))
            })
            .take(MEANING_ROWS)
            .collect();
        if cancel.is_cancelled() {
            return rows;
        }
        let typed = display_form(text);
        rows.extend(
            dictionary
                .suggestions(text, SUGGESTION_ROWS + 1)
                .into_iter()
                .filter(|word| *word != typed)
                .take(SUGGESTION_ROWS)
                .map(suggestion_row),
        );
        if rows.is_empty() {
            rows.push(not_found_row(&typed));
        }
        rows
    }
}

fn meaning_row(entry: &Entry, meaning: &Meaning) -> Row {
    let mut subtitle = format!("{}, {}", entry.word, meaning.word_class);
    if !meaning.synonyms.is_empty() {
        subtitle.push_str(". Synonyms: ");
        subtitle.push_str(&meaning.synonyms.join(", "));
    }
    Row::new(
        RowKind::Dictionary,
        meaning.definition.clone(),
        Action::OpenDictionary(entry.word.clone()),
    )
    .with_subtitle(subtitle)
    .with_icon(dictionary_icon())
}

fn suggestion_row(word: String) -> Row {
    Row::new(
        RowKind::Dictionary,
        word.clone(),
        Action::OpenDictionary(word),
    )
    .with_subtitle("Open in the dictionary")
    .with_icon(dictionary_icon())
}

fn not_found_row(typed: &str) -> Row {
    Row::new(
        RowKind::Dictionary,
        format!("\"{typed}\" is not in the dictionary"),
        Action::Nothing,
    )
    .with_subtitle("No word starts with this text")
    .with_icon(dictionary_icon())
}

/// The row for a dictionary that cannot open. When the data is not
/// installed, the row tells the install command and the path that was
/// tried, and Enter copies the command.
fn error_row(error: &Error) -> Row {
    let row = match error {
        Error::NotInstalled { .. } | Error::Incomplete { .. } => Row::new(
            RowKind::Dictionary,
            format!("Install the dictionary: {INSTALL_COMMAND}"),
            Action::Copy(INSTALL_COMMAND.to_owned()),
        )
        .with_subtitle(format!("{}. Enter copies the command", capitalized(error))),
        Error::Read { .. } | Error::Format { .. } => Row::new(
            RowKind::Dictionary,
            "The dictionary cannot be read",
            Action::Nothing,
        )
        .with_subtitle(capitalized(error)),
    };
    row.with_icon(Icon::Name("dialog-warning".to_owned()))
}

fn dictionary_icon() -> Icon {
    Icon::Name("accessories-dictionary".to_owned())
}

/// The form of the words of the dictionary: lower case, one space between
/// the words.
fn display_form(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn capitalized(error: &Error) -> String {
    let text = error.to_string();
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => text,
    }
}

/// `~` and `~/...` are below the home directory.
fn expand_home(setting: &str, home: &Path) -> PathBuf {
    let setting = setting.trim();
    if setting == "~" {
        home.to_owned()
    } else if let Some(rest) = setting.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(setting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_directory_gives_one_row_with_the_install_command_and_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("wordnet");
        let provider = DictionaryProvider(Arc::new(DictionaryState::new(
            dir.path(),
            missing.to_str().unwrap(),
        )));

        let rows = provider.search("launcher", &Settings::default(), &CancelToken::new());

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, RowKind::Dictionary);
        assert!(rows[0].title.contains(INSTALL_COMMAND), "{}", rows[0].title);
        assert!(
            rows[0].subtitle.contains(missing.to_str().unwrap()),
            "{}",
            rows[0].subtitle
        );
        assert_eq!(rows[0].enter, Action::Copy(INSTALL_COMMAND.into()));
    }

    #[test]
    fn a_wordnet_file_that_cannot_be_read_gives_an_error_row_that_does_nothing() {
        let dir = tempfile::tempdir().unwrap();
        for class in ["noun", "verb", "adj", "adv"] {
            for kind in ["index", "data"] {
                std::fs::write(dir.path().join(format!("{kind}.{class}")), "").unwrap();
            }
            std::fs::write(dir.path().join(format!("{class}.exc")), "").unwrap();
        }
        std::fs::write(dir.path().join("index.noun"), "bad line\n").unwrap();
        let state = DictionaryState::new(dir.path(), dir.path().to_str().unwrap());

        let rows = DictionaryProvider(Arc::new(state)).search(
            "x",
            &Settings::default(),
            &CancelToken::new(),
        );

        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].title, "The dictionary cannot be read");
        assert_eq!(rows[0].enter, Action::Nothing);
    }

    #[test]
    fn a_new_path_closes_the_old_dictionary_and_a_failure_can_be_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let state = DictionaryState::new(dir.path(), "~/wordnet");
        assert!(matches!(
            state.get().map(|_| ()).unwrap_err().as_ref(),
            Error::NotInstalled { path } if *path == dir.path().join("wordnet")
        ));

        state.set_path("~/other");
        let after_set_path = state.get().map(|_| ()).unwrap_err();
        state.forget_failure();
        let after_forget = state.get().map(|_| ()).unwrap_err();

        assert!(matches!(
            after_set_path.as_ref(),
            Error::NotInstalled { path } if *path == dir.path().join("other")
        ));
        assert!(!Arc::ptr_eq(&after_set_path, &after_forget));
    }

    #[test]
    fn the_home_directory_expands_in_the_setting() {
        let home = Path::new("/home/u");

        assert_eq!(expand_home("~", home), PathBuf::from("/home/u"));
        assert_eq!(expand_home(" ~/wn ", home), PathBuf::from("/home/u/wn"));
        assert_eq!(
            expand_home("/usr/share/wordnet", home),
            PathBuf::from("/usr/share/wordnet")
        );
    }

    #[test]
    fn the_typed_text_gets_the_form_of_the_dictionary_words() {
        assert_eq!(display_form("  Launch   Pad "), "launch pad");
    }
}
