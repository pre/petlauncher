use std::path::PathBuf;

use petlauncher_dict::Dictionary;

fn suggestions(text: &str, limit: usize) -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wordnet");
    let dictionary = Dictionary::open(path).expect("the fixture dictionary");
    dictionary.suggestions(text, limit)
}

#[test]
fn typed_word_first_then_frequent_words_then_alphabetical_order() {
    let words = suggestions("launch", 10);

    assert_eq!(
        words,
        [
            "launch",
            "launcher",
            "launching",
            "launch pad",
            "launching pad",
            "launchpad"
        ]
    );
}

#[test]
fn limit_keeps_the_first_words_of_the_order() {
    assert_eq!(suggestions("launch", 2), ["launch", "launcher"]);
    assert_eq!(suggestions("launch", 4).len(), 4);
}

#[test]
fn prefix_ignores_case() {
    assert_eq!(suggestions("LAUNCHI", 10), ["launching", "launching pad"]);
}

#[test]
fn space_at_the_end_asks_for_collocations() {
    assert_eq!(suggestions("launch ", 10), ["launch pad"]);
    assert_eq!(suggestions("new york ", 10), ["new york city"]);
}

#[test]
fn empty_text_or_zero_limit_gives_no_words() {
    assert!(suggestions("", 10).is_empty());
    assert!(suggestions("  ", 10).is_empty());
    assert!(suggestions("launch", 0).is_empty());
}

#[test]
fn prefix_of_no_word_gives_no_words() {
    assert!(suggestions("xyz", 10).is_empty());
}
