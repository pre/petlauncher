//! Where the dictionary window gets its words: the trait `DictionarySource`.
//! The program gives the `Launcher` (`backend::LauncherDictionary`), so the
//! launcher rows and the window use one open dictionary.

use std::sync::Arc;

use petlauncher_dict::{Entry, Error};

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
