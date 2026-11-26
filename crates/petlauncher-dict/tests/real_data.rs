//! Tests with the real WordNet data. They run only when the environment
//! variable `PETLAUNCHER_WORDNET_PATH` gives the WordNet directory, for example
//! `/usr/share/wordnet`. Without it, each test passes and does nothing.

use petlauncher_dict::{Dictionary, Found, WordClass};

fn real_dictionary() -> Option<Dictionary> {
    let path = std::env::var_os("PETLAUNCHER_WORDNET_PATH")?;
    Some(Dictionary::open(path).expect("the real WordNet data"))
}

#[test]
fn real_launch_gives_suggestions_that_start_with_launch() {
    let Some(dictionary) = real_dictionary() else {
        return;
    };

    let words = dictionary.suggestions("launch", 10);

    assert_eq!(words.first().map(String::as_str), Some("launch"));
    assert!(words.contains(&"launcher".to_owned()), "{words:?}");
    assert!(
        words.iter().all(|word| word.starts_with("launch")),
        "{words:?}"
    );
}

#[test]
fn real_launcher_gives_two_noun_meanings_with_synonyms() {
    let Some(dictionary) = real_dictionary() else {
        return;
    };

    let entries = dictionary.lookup("launcher").expect("a lookup");

    assert_eq!(entries.len(), 1);
    let meanings = &entries[0].meanings;
    assert_eq!(meanings.len(), 2);
    assert!(meanings.iter().all(|m| m.word_class == WordClass::Noun));
    let synonyms: Vec<&str> = meanings
        .iter()
        .flat_map(|m| &m.synonyms)
        .map(String::as_str)
        .collect();
    assert_eq!(synonyms, ["rocket launcher", "catapult"]);
}

#[test]
fn real_inflected_forms_find_their_base_forms() {
    let Some(dictionary) = real_dictionary() else {
        return;
    };

    for (text, base) in [
        ("launchers", "launcher"),
        ("ran", "run"),
        ("better", "good"),
    ] {
        let entries = dictionary.lookup(text).expect("a lookup");

        let found = entries
            .iter()
            .find(|entry| entry.word == base)
            .unwrap_or_else(|| panic!("{text} gives no {base}"));
        assert_eq!(found.found, Found::BaseForm, "{text}");
    }
}

#[test]
fn real_related_words_of_common_words_are_words_of_the_dictionary() {
    let Some(dictionary) = real_dictionary() else {
        return;
    };

    for text in [
        "launcher", "launch", "run", "good", "set", "dog", "new york",
    ] {
        for entry in dictionary.lookup(text).expect("a lookup") {
            for meaning in entry.meanings {
                let related = [
                    meaning.synonyms,
                    meaning.antonyms,
                    meaning.see_also,
                    meaning.hypernyms,
                    meaning.hyponyms,
                ];
                for word in related.iter().flatten() {
                    let found = dictionary.lookup(word).expect("a lookup");
                    assert!(
                        found.first().is_some_and(|e| e.found == Found::Exact),
                        "{word} of {text}"
                    );
                }
            }
        }
    }
}
