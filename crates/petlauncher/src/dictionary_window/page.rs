//! What the dictionary window shows for a word, and which word the search
//! field opens. Plain logic, with no widgets.

use petlauncher_dict::{Entry, Error, Meaning};

use super::source::{DictionarySource, SourceError};

/// The number of similar words on the "not found" page.
pub const NEAREST: usize = 8;

/// The number of words that `nearest` compares with the typed word.
const CANDIDATES: usize = 300;

/// The command that installs the WordNet data on Ubuntu.
pub const INSTALL_COMMAND: &str = "sudo apt install wordnet-base";

/// The command that installs the WordNet data again when its files are bad.
pub const REINSTALL_COMMAND: &str = "sudo apt install --reinstall wordnet-base";

/// The contents of the window for one word.
#[derive(Debug, Clone)]
pub enum Page {
    /// The word and its base forms. `entries` is not empty.
    Word { word: String, entries: Vec<Entry> },
    /// The dictionary does not have the word. `nearest` are similar words.
    NotFound { word: String, nearest: Vec<String> },
    /// The dictionary data is missing or bad. The page is the same for
    /// each word.
    Unavailable(SourceError),
}

impl Page {
    pub fn for_word(source: &dyn DictionarySource, word: &str) -> Page {
        let word = word.to_owned();
        match source.lookup(&word) {
            Ok(entries) if !entries.is_empty() => Page::Word { word, entries },
            Ok(_) => match nearest(source, &word, NEAREST) {
                Ok(nearest) => Page::NotFound { word, nearest },
                Err(error) => Page::Unavailable(error),
            },
            Err(error) => Page::Unavailable(error),
        }
    }

    /// The number of meanings, for the log.
    pub fn meanings(&self) -> usize {
        match self {
            Page::Word { entries, .. } => entries.iter().map(|entry| entry.meanings.len()).sum(),
            _ => 0,
        }
    }
}

/// The form of a word in the history: no spaces at the ends, one space
/// between the parts, lower case. The dictionary finds a word in each case,
/// so `New York` and `new york` are the same visit.
pub fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The meanings of one word class, in the order of the dictionary.
/// `petlauncher-dict` gives the meanings class by class, so each class is
/// one group.
pub fn groups(meanings: &[Meaning]) -> Vec<&[Meaning]> {
    meanings
        .chunk_by(|a, b| a.word_class == b.word_class)
        .collect()
}

/// The word that Enter in the search field opens: the typed text when the
/// dictionary finds it (also as an inflected form, for example `ran`), else
/// the first word that starts with the text, else the typed text, which
/// then shows the "not found" page. `None` for an empty field.
pub fn search_target(source: &dyn DictionarySource, text: &str) -> Option<String> {
    let word = normalize(text);
    if word.is_empty() {
        return None;
    }
    let found = match source.lookup(&word) {
        Ok(entries) => !entries.is_empty(),
        // The page of the problem shows for the typed text.
        Err(_) => true,
    };
    if found {
        return Some(word);
    }
    let first = source
        .suggestions(&word, 1)
        .ok()
        .and_then(|words| words.into_iter().next());
    Some(first.unwrap_or(word))
}

/// At most `limit` words that are similar to `word`. The trait gives only
/// the words with a prefix, so the candidates are the words that start with
/// the first half of `word`. The candidates with the smallest edit distance
/// come first, and equal distances keep the order of the dictionary (the
/// more frequent words first). Thus `launchr` gives `launcher` and `launch`,
/// and `recieve` gives `receive`.
pub fn nearest(
    source: &dyn DictionarySource,
    word: &str,
    limit: usize,
) -> Result<Vec<String>, SourceError> {
    let chars: Vec<char> = word.chars().collect();
    let prefix: String = chars[..(chars.len() / 2).max(1).min(chars.len())]
        .iter()
        .collect();
    if prefix.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut candidates = source.suggestions(&prefix, CANDIDATES)?;
    candidates.retain(|candidate| candidate != word);
    candidates.sort_by_cached_key(|candidate| edit_distance(word, candidate));
    candidates.truncate(limit);
    Ok(candidates)
}

/// The number of character insertions, removals, changes and exchanges of
/// two adjacent characters that change `a` into `b` (the optimal string
/// alignment distance).
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    // rows[i][j] is the distance of a[..i] and b[..j].
    let mut rows = vec![vec![0; b.len() + 1]; a.len() + 1];
    for (i, row) in rows.iter_mut().enumerate() {
        row[0] = i;
    }
    rows[0] = (0..=b.len()).collect();
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let change = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (rows[i - 1][j] + 1)
                .min(rows[i][j - 1] + 1)
                .min(rows[i - 1][j - 1] + change);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(rows[i - 2][j - 2] + 1);
            }
            rows[i][j] = best;
        }
    }
    rows[a.len()][b.len()]
}

/// The text of the page for missing or bad dictionary data (R19: the cause,
/// the path that was tried and the install command).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub title: &'static str,
    pub cause: String,
    pub path: String,
    pub command: &'static str,
}

impl Problem {
    pub fn of(error: &Error) -> Problem {
        match error {
            Error::NotInstalled { path } => Problem {
                title: "The dictionary is not installed",
                cause: "The directory of the WordNet data does not exist.".to_owned(),
                path: path.display().to_string(),
                command: INSTALL_COMMAND,
            },
            Error::Incomplete { path, missing } => Problem {
                title: "The dictionary is not installed",
                cause: format!(
                    "The directory of the WordNet data does not have these files: {}.",
                    missing.join(", ")
                ),
                path: path.display().to_string(),
                command: INSTALL_COMMAND,
            },
            Error::Read { path, source } => Problem {
                title: "The dictionary cannot be read",
                cause: format!("A file of the WordNet data cannot be read: {source}."),
                path: path.display().to_string(),
                command: REINSTALL_COMMAND,
            },
            Error::Format { path, detail } => Problem {
                title: "The dictionary cannot be read",
                cause: format!("A file does not have the WordNet format: {detail}."),
                path: path.display().to_string(),
                command: REINSTALL_COMMAND,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary_window::source::fake::{entry, meaning, FakeSource};
    use petlauncher_dict::{Found, WordClass};
    use std::path::PathBuf;

    #[test]
    fn a_word_of_the_dictionary_gives_its_entries() {
        let source = FakeSource::with_words(&["launcher"]);

        let page = Page::for_word(&source, "launcher");

        assert!(
            matches!(&page, Page::Word { word, entries } if word == "launcher" && entries.len() == 1)
        );
        assert_eq!(page.meanings(), 1);
    }

    #[test]
    fn a_word_with_only_a_base_form_is_a_word_page() {
        let mut source = FakeSource::with_words(&["run"]);
        source
            .entries
            .insert("ran".into(), vec![entry("run", Found::BaseForm)]);

        let page = Page::for_word(&source, "ran");

        assert!(
            matches!(&page, Page::Word { word, entries } if word == "ran" && entries[0].word == "run")
        );
    }

    #[test]
    fn an_unknown_word_gives_the_nearest_words() {
        let source = FakeSource::with_words(&["launch", "laundry", "launcher", "laugh"]);

        let page = Page::for_word(&source, "launchr");

        match page {
            Page::NotFound { word, nearest } => {
                assert_eq!(word, "launchr");
                assert_eq!(nearest[..2], ["launch", "launcher"]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn missing_data_gives_the_error() {
        let source = FakeSource::missing();

        let page = Page::for_word(&source, "launcher");

        assert!(
            matches!(page, Page::Unavailable(error) if matches!(*error, Error::NotInstalled { .. }))
        );
    }

    #[test]
    fn normalize_gives_one_form_for_the_history() {
        assert_eq!(normalize("  New   York "), "new york");
        assert_eq!(normalize("launcher"), "launcher");
        assert_eq!(normalize(" \t"), "");
    }

    #[test]
    fn groups_keep_the_order_of_the_classes() {
        let meanings = vec![
            meaning(WordClass::Verb, "v1"),
            meaning(WordClass::Verb, "v2"),
            meaning(WordClass::Noun, "n1"),
            meaning(WordClass::Adjective, "a1"),
        ];

        let groups = groups(&meanings);

        let classes: Vec<(WordClass, usize)> = groups
            .iter()
            .map(|group| (group[0].word_class, group.len()))
            .collect();
        assert_eq!(
            classes,
            [
                (WordClass::Verb, 2),
                (WordClass::Noun, 1),
                (WordClass::Adjective, 1)
            ]
        );
    }

    #[test]
    fn groups_of_no_meanings_is_empty() {
        assert!(groups(&[]).is_empty());
    }

    #[test]
    fn enter_opens_the_typed_word() {
        let source = FakeSource::with_words(&["launch", "launcher"]);

        assert_eq!(
            search_target(&source, " Launcher ").as_deref(),
            Some("launcher")
        );
    }

    #[test]
    fn enter_opens_an_inflected_form_that_the_dictionary_finds() {
        let mut source = FakeSource::with_words(&["range", "run"]);
        source
            .entries
            .insert("ran".into(), vec![entry("run", Found::BaseForm)]);

        assert_eq!(search_target(&source, "ran").as_deref(), Some("ran"));
    }

    #[test]
    fn enter_on_the_start_of_a_word_opens_the_first_suggestion() {
        let source = FakeSource::with_words(&["launch", "launcher"]);

        assert_eq!(search_target(&source, "laun").as_deref(), Some("launch"));
    }

    #[test]
    fn enter_on_an_unknown_text_opens_it_for_the_not_found_page() {
        let source = FakeSource::with_words(&["launch"]);

        assert_eq!(search_target(&source, "xyzzy").as_deref(), Some("xyzzy"));
    }

    #[test]
    fn enter_in_an_empty_field_opens_nothing() {
        let source = FakeSource::with_words(&["launch"]);

        assert_eq!(search_target(&source, "  "), None);
    }

    #[test]
    fn enter_with_missing_data_opens_the_text_for_the_problem_page() {
        let source = FakeSource::missing();

        assert_eq!(
            search_target(&source, "launcher").as_deref(),
            Some("launcher")
        );
    }

    #[test]
    fn nearest_finds_an_exchange_of_two_letters() {
        let source = FakeSource::with_words(&["recall", "record", "receive", "recent"]);

        let words = nearest(&source, "recieve", 2).unwrap();

        assert_eq!(words[0], "receive");
    }

    #[test]
    fn nearest_keeps_the_order_of_the_dictionary_for_equal_distances() {
        let source = FakeSource::with_words(&["launches", "launched"]);

        let words = nearest(&source, "launchez", 2).unwrap();

        assert_eq!(words, ["launches", "launched"]);
    }

    #[test]
    fn nearest_of_one_letter_uses_that_letter() {
        let source = FakeSource::with_words(&["q", "quite"]);

        let words = nearest(&source, "x", 3).unwrap();

        assert!(words.is_empty());
    }

    #[test]
    fn nearest_of_an_empty_word_is_empty() {
        let source = FakeSource::with_words(&["a"]);

        assert!(nearest(&source, "", 3).unwrap().is_empty());
    }

    #[test]
    fn edit_distance_counts_each_change() {
        assert_eq!(edit_distance("launcher", "launcher"), 0);
        assert_eq!(edit_distance("launchr", "launcher"), 1);
        assert_eq!(edit_distance("launchr", "launch"), 1);
        assert_eq!(edit_distance("recieve", "receive"), 1);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn a_missing_directory_gives_the_install_command() {
        let problem = Problem::of(&Error::NotInstalled {
            path: PathBuf::from("/usr/share/wordnet"),
        });

        assert_eq!(problem.title, "The dictionary is not installed");
        assert_eq!(problem.path, "/usr/share/wordnet");
        assert_eq!(problem.command, "sudo apt install wordnet-base");
    }

    #[test]
    fn missing_files_are_named_in_the_cause() {
        let problem = Problem::of(&Error::Incomplete {
            path: PathBuf::from("/wn"),
            missing: vec!["data.noun", "noun.exc"],
        });

        assert!(
            problem.cause.contains("data.noun, noun.exc"),
            "{}",
            problem.cause
        );
        assert_eq!(problem.command, INSTALL_COMMAND);
    }

    #[test]
    fn bad_files_give_the_reinstall_command() {
        let format = Problem::of(&Error::Format {
            path: PathBuf::from("/wn/data.noun"),
            detail: "no gloss".into(),
        });
        let read = Problem::of(&Error::Read {
            path: PathBuf::from("/wn/data.noun"),
            source: std::io::Error::other("disk"),
        });

        assert_eq!(format.title, "The dictionary cannot be read");
        assert_eq!(format.command, REINSTALL_COMMAND);
        assert!(read.cause.contains("disk"));
        assert_eq!(read.path, "/wn/data.noun");
    }
}
