//! The acceptance steps of R19 that the crate can do without a user interface.

use std::path::PathBuf;

use petlauncher_dict::{Dictionary, Found, WordClass};

fn dictionary() -> Dictionary {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wordnet");
    Dictionary::open(path).expect("the fixture dictionary")
}

/// Step 1: `d launch` shows word suggestions that start with "launch".
#[test]
fn r19_launch_as_prefix_gives_word_suggestions_that_start_with_launch() {
    let words = dictionary().suggestions("launch", 10);

    assert!(words.len() > 1, "{words:?}");
    assert!(
        words.iter().all(|word| word.starts_with("launch")),
        "{words:?}"
    );
    assert!(words.contains(&"launcher".to_owned()), "{words:?}");
}

/// Step 2: `d launcher` shows the meanings of "launcher", each with its word
/// class and synonyms.
#[test]
fn r19_launcher_gives_its_meanings_with_word_class_and_synonyms() {
    let entries = dictionary().lookup("launcher").expect("a lookup");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].found, Found::Exact);
    let meanings = &entries[0].meanings;
    assert_eq!(meanings.len(), 2);
    assert!(meanings.iter().all(|m| m.word_class == WordClass::Noun));
    assert!(meanings.iter().all(|m| !m.synonyms.is_empty()));
    assert!(meanings.iter().all(|m| !m.definition.is_empty()));
}

/// Step 5: a synonym is a link that opens that word in the window.
#[test]
fn r19_each_synonym_of_launcher_opens_a_word() {
    let dictionary = dictionary();
    let entries = dictionary.lookup("launcher").expect("a lookup");

    for synonym in entries[0].meanings.iter().flat_map(|m| &m.synonyms) {
        let found = dictionary.lookup(synonym).expect("a lookup");
        assert_eq!(found[0].found, Found::Exact, "{synonym}");
        assert_eq!(&found[0].word, synonym);
    }
}
