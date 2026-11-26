//! Where the dictionary window gets its words: the trait `DictionarySource`
//! and the implementation `WordNet`, which opens the WordNet data of the
//! setting `dictionary.wordnet_path` itself.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use petlauncher_dict::{Dictionary, Entry, Error};

use crate::log;

/// An error of the dictionary data, for example a missing WordNet directory.
/// `Arc`, because the `Launcher` of `petlauncher-providers` keeps a failed
/// open and gives the same error to each caller.
pub type SourceError = Arc<Error>;

/// The two functions of the dictionary that the window uses. They are the
/// functions `dictionary_lookup` and `dictionary_suggestions` of the
/// `Launcher` in `petlauncher-providers`.
///
/// The window calls them on the main thread: a lookup takes less than 1 ms
/// when the dictionary is open (`docs/dictionary-window.md`).
pub trait DictionarySource {
    /// The entries of the word and of its base forms, as
    /// `Dictionary::lookup` gives them. No entries: the word is not in the
    /// dictionary.
    fn lookup(&self, word: &str) -> Result<Vec<Entry>, SourceError>;

    /// At most `limit` words that start with `text`, as
    /// `Dictionary::suggestions` gives them.
    fn suggestions(&self, text: &str, limit: usize) -> Result<Vec<String>, SourceError>;
}

/// Opens the WordNet data of the setting `dictionary.wordnet_path` at the
/// first use, and again when the setting changes. `~` in the setting is the
/// home directory.
///
/// A failed open is not kept: each call tries again, so the window works
/// after the owner installs the data. A missing directory fails after one
/// `stat` call, so this costs nothing.
pub struct WordNet {
    home: PathBuf,
    setting: Box<dyn Fn() -> String>,
    open: RefCell<Option<(PathBuf, Dictionary)>>,
}

impl WordNet {
    /// `setting` gives the value of `dictionary.wordnet_path` now.
    pub fn new(home: PathBuf, setting: impl Fn() -> String + 'static) -> Self {
        Self {
            home,
            setting: Box::new(setting),
            open: RefCell::new(None),
        }
    }

    fn with_dictionary<T>(&self, use_it: impl FnOnce(&Dictionary) -> T) -> Result<T, SourceError> {
        let path = expand_home(&(self.setting)(), &self.home);
        let mut open = self.open.borrow_mut();
        if !matches!(&*open, Some((open_path, _)) if *open_path == path) {
            let start = Instant::now();
            let dictionary = Dictionary::open(&path).map_err(Arc::new)?;
            log::debug(format_args!(
                "dictionary window: opened {} in {:.1} ms",
                path.display(),
                start.elapsed().as_secs_f64() * 1000.0
            ));
            *open = Some((path, dictionary));
        }
        let (_, dictionary) = open.as_ref().expect("the dictionary is open");
        Ok(use_it(dictionary))
    }
}

impl DictionarySource for WordNet {
    fn lookup(&self, word: &str) -> Result<Vec<Entry>, SourceError> {
        self.with_dictionary(|dictionary| dictionary.lookup(word))?
            .map_err(Arc::new)
    }

    fn suggestions(&self, text: &str, limit: usize) -> Result<Vec<String>, SourceError> {
        self.with_dictionary(|dictionary| dictionary.suggestions(text, limit))
    }
}

/// The same rule as the `Launcher` of `petlauncher-providers`.
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

/// A source in memory for the tests of the window logic.
#[cfg(test)]
pub mod fake {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    use petlauncher_dict::{Entry, Error, Found, Meaning, WordClass};

    use super::{DictionarySource, SourceError};

    #[derive(Default)]
    pub struct FakeSource {
        /// The words in the order of the suggestions.
        pub words: Vec<String>,
        pub entries: HashMap<String, Vec<Entry>>,
        pub missing: bool,
    }

    impl FakeSource {
        /// Each word is a noun with one meaning, and the order of the list
        /// is the order of the suggestions.
        pub fn with_words(words: &[&str]) -> Self {
            let mut source = FakeSource::default();
            for word in words {
                source.words.push((*word).to_owned());
                source
                    .entries
                    .insert((*word).to_owned(), vec![entry(word, Found::Exact)]);
            }
            source
        }

        pub fn missing() -> Self {
            FakeSource {
                missing: true,
                ..FakeSource::default()
            }
        }

        fn check(&self) -> Result<(), SourceError> {
            if self.missing {
                return Err(Arc::new(Error::NotInstalled {
                    path: PathBuf::from("/usr/share/wordnet"),
                }));
            }
            Ok(())
        }
    }

    pub fn meaning(word_class: WordClass, definition: &str) -> Meaning {
        Meaning {
            word_class,
            definition: definition.to_owned(),
            examples: Vec::new(),
            synonyms: Vec::new(),
            antonyms: Vec::new(),
            see_also: Vec::new(),
            hypernyms: Vec::new(),
            hyponyms: Vec::new(),
        }
    }

    pub fn entry(word: &str, found: Found) -> Entry {
        Entry {
            word: word.to_owned(),
            found,
            meanings: vec![meaning(WordClass::Noun, &format!("the meaning of {word}"))],
        }
    }

    impl DictionarySource for FakeSource {
        fn lookup(&self, word: &str) -> Result<Vec<Entry>, SourceError> {
            self.check()?;
            Ok(self.entries.get(word).cloned().unwrap_or_default())
        }

        fn suggestions(&self, text: &str, limit: usize) -> Result<Vec<String>, SourceError> {
            self.check()?;
            Ok(self
                .words
                .iter()
                .filter(|word| word.starts_with(text))
                .take(limit)
                .cloned()
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../petlauncher-dict/tests/fixtures/wordnet")
    }

    #[test]
    fn looks_up_a_word_in_the_directory_of_the_setting() {
        let path = fixture().to_string_lossy().into_owned();
        let wordnet = WordNet::new(PathBuf::from("/home/test"), move || path.clone());

        let entries = wordnet.lookup("launcher").unwrap();

        assert_eq!(entries[0].word, "launcher");
        assert_eq!(entries[0].meanings.len(), 2);
    }

    #[test]
    fn gives_the_suggestions_of_the_dictionary() {
        let path = fixture().to_string_lossy().into_owned();
        let wordnet = WordNet::new(PathBuf::from("/home/test"), move || path.clone());

        let words = wordnet.suggestions("launch", 3).unwrap();

        assert_eq!(words[0], "launch");
        assert_eq!(words.len(), 3);
    }

    #[test]
    fn a_tilde_in_the_setting_is_the_home_directory() {
        let fixture = fixture();
        let home = fixture.parent().unwrap().to_owned();
        let wordnet = WordNet::new(home, || "~/wordnet".to_owned());

        let entries = wordnet.lookup("launcher").unwrap();

        assert_eq!(entries[0].word, "launcher");
    }

    #[test]
    fn a_missing_directory_gives_the_error_with_the_path() {
        let wordnet = WordNet::new(PathBuf::from("/home/test"), || {
            "/nonexistent/wordnet".to_owned()
        });

        let error = wordnet.lookup("launcher").unwrap_err();

        assert!(
            matches!(&*error, Error::NotInstalled { path } if path == Path::new("/nonexistent/wordnet")),
            "{error}"
        );
    }

    #[test]
    fn opens_the_new_directory_after_a_change_of_the_setting() {
        let use_fixture = Rc::new(Cell::new(false));
        let setting = Rc::clone(&use_fixture);
        let path = fixture().to_string_lossy().into_owned();
        let wordnet = WordNet::new(PathBuf::from("/home/test"), move || {
            if setting.get() {
                path.clone()
            } else {
                "/nonexistent/wordnet".to_owned()
            }
        });
        let before = wordnet.lookup("launcher");

        use_fixture.set(true);
        let after = wordnet.lookup("launcher");

        assert!(before.is_err());
        assert_eq!(after.unwrap()[0].word, "launcher");
    }

    #[test]
    fn expands_only_a_tilde_at_the_start() {
        let home = Path::new("/home/test");

        assert_eq!(expand_home("~", home), PathBuf::from("/home/test"));
        assert_eq!(expand_home(" ~/wn ", home), PathBuf::from("/home/test/wn"));
        assert_eq!(
            expand_home("/usr/share/wordnet", home),
            PathBuf::from("/usr/share/wordnet")
        );
        assert_eq!(expand_home("a~/b", home), PathBuf::from("a~/b"));
    }
}
