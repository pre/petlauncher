use std::path::PathBuf;

use petlauncher_dict::{Dictionary, Entry, Found, Meaning, WordClass};

fn dictionary() -> Dictionary {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/wordnet");
    Dictionary::open(path).expect("the fixture dictionary")
}

fn lookup(text: &str) -> Vec<Entry> {
    dictionary().lookup(text).expect("a lookup")
}

fn words(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.word.as_str()).collect()
}

fn classes(meanings: &[Meaning]) -> Vec<WordClass> {
    meanings.iter().map(|meaning| meaning.word_class).collect()
}

fn only_meaning(text: &str) -> Meaning {
    let mut entries = lookup(text);
    assert_eq!(entries.len(), 1, "{entries:?}");
    let mut meanings = entries.remove(0).meanings;
    assert_eq!(meanings.len(), 1, "{meanings:?}");
    meanings.remove(0)
}

#[test]
fn word_gives_its_meanings_with_class_definition_examples_and_synonyms() {
    let entries = lookup("launcher");

    assert_eq!(words(&entries), ["launcher"]);
    let entry = &entries[0];
    assert_eq!(entry.found, Found::Exact);
    assert_eq!(classes(&entry.meanings), [WordClass::Noun, WordClass::Noun]);
    let first = &entry.meanings[0];
    assert_eq!(first.definition, "a device that fires rockets");
    assert_eq!(first.examples, ["the soldiers carried a launcher"]);
    assert_eq!(first.synonyms, ["rocket launcher"]);
    assert_eq!(entry.meanings[1].synonyms, ["catapult"]);
}

#[test]
fn lookup_ignores_case_and_spaces_around_the_word() {
    assert_eq!(lookup("  LAUNCHER \n"), lookup("launcher"));
}

#[test]
fn collocation_has_spaces_and_keeps_the_case_of_its_synonyms() {
    let entries = lookup("New   York");

    assert_eq!(words(&entries), ["new york"]);
    assert_eq!(
        entries[0].meanings[0].synonyms,
        ["New York City", "Greater New York"]
    );
}

#[test]
fn unknown_word_or_empty_text_gives_no_entries() {
    assert!(lookup("xyzzy").is_empty());
    assert!(lookup("   ").is_empty());
}

#[test]
fn class_with_more_tagged_senses_comes_first() {
    assert_eq!(
        classes(&lookup("launch")[0].meanings),
        [WordClass::Verb, WordClass::Verb, WordClass::Noun]
    );
    assert_eq!(
        classes(&lookup("good")[0].meanings),
        [
            WordClass::Adjective,
            WordClass::Adjective,
            WordClass::Noun,
            WordClass::Adverb
        ]
    );
}

#[test]
fn senses_of_one_class_keep_the_order_of_the_index() {
    let entries = lookup("launch");

    let definitions: Vec<&str> = entries[0]
        .meanings
        .iter()
        .map(|meaning| meaning.definition.as_str())
        .collect();

    assert_eq!(
        definitions,
        [
            "send into the air with force",
            "start a new thing",
            "a small boat with a motor"
        ]
    );
}

#[test]
fn plural_finds_the_singular_as_its_base_form() {
    let entries = lookup("launchers");

    assert_eq!(words(&entries), ["launcher"]);
    assert_eq!(entries[0].found, Found::BaseForm);
    assert_eq!(entries[0].meanings.len(), 2);
}

#[test]
fn irregular_verb_finds_its_base_form_with_the_verb_meanings_only() {
    let entries = lookup("ran");

    assert_eq!(words(&entries), ["run"]);
    assert_eq!(entries[0].found, Found::BaseForm);
    assert_eq!(
        classes(&entries[0].meanings),
        [WordClass::Verb, WordClass::Verb]
    );
}

#[test]
fn better_finds_itself_and_its_base_forms_good_and_well() {
    let entries = lookup("better");

    assert_eq!(words(&entries), ["better", "good", "well"]);
    let found: Vec<Found> = entries.iter().map(|entry| entry.found).collect();
    assert_eq!(found, [Found::Exact, Found::BaseForm, Found::BaseForm]);
    assert_eq!(
        classes(&entries[0].meanings),
        [WordClass::Adjective, WordClass::Adverb, WordClass::Verb]
    );
    assert_eq!(
        classes(&entries[1].meanings),
        [WordClass::Adjective, WordClass::Adjective]
    );
    assert_eq!(
        classes(&entries[2].meanings),
        [WordClass::Adjective, WordClass::Adverb]
    );
}

#[test]
fn word_that_is_also_an_inflection_gives_the_word_and_its_base_form() {
    let entries = lookup("glasses");

    assert_eq!(words(&entries), ["glasses", "glass"]);
    assert_eq!(entries[0].found, Found::Exact);
    assert_eq!(entries[1].found, Found::BaseForm);
}

#[test]
fn irregular_plural_with_two_base_forms_gives_both() {
    assert_eq!(words(&lookup("axes")), ["ax", "axis"]);
}

#[test]
fn inflected_collocation_finds_its_base_form() {
    assert_eq!(words(&lookup("looked up")), ["look up"]);
}

#[test]
fn adjective_markers_are_not_part_of_the_words() {
    assert_eq!(only_meaning("galore").synonyms, ["abundant"]);
    assert_eq!(only_meaning("abundant").synonyms, ["galore"]);
    assert_eq!(only_meaning("afraid").word_class, WordClass::Adjective);
    assert_eq!(only_meaning("former").definition, "earlier in time");
}

#[test]
fn satellite_adjective_is_an_adjective() {
    let entries = lookup("full");

    let meaning = &entries[0].meanings[0];
    assert_eq!(meaning.word_class, WordClass::Adjective);
    assert_eq!(meaning.synonyms, ["good"]);
    assert_eq!(meaning.examples, ["a full cup"]);
}

#[test]
fn antonym_pointers_give_antonyms() {
    assert_eq!(lookup("good")[0].meanings[0].antonyms, ["bad"]);
    assert_eq!(only_meaning("bad").antonyms, ["good"]);
}

#[test]
fn antonym_of_one_word_of_a_synset_is_not_an_antonym_of_its_other_words() {
    assert_eq!(only_meaning("large").antonyms, ["small"]);
    assert!(only_meaning("big").antonyms.is_empty());
    assert_eq!(only_meaning("small").antonyms, ["large"]);
}

#[test]
fn see_also_pointer_to_one_word_gives_that_word() {
    let entries = lookup("run");

    assert_eq!(entries[0].meanings[0].see_also, ["run away"]);
}

#[test]
fn hypernyms_and_hyponyms_give_the_words_of_their_synsets() {
    let launcher = lookup("launcher");

    assert_eq!(launcher[0].meanings[0].hypernyms, ["device"]);
    assert_eq!(
        only_meaning("device").hyponyms,
        ["launcher", "rocket launcher", "catapult"]
    );
}

#[test]
fn other_pointers_are_not_relations() {
    let meaning = &lookup("launcher")[0].meanings[0];

    assert!(meaning.antonyms.is_empty());
    assert!(meaning.see_also.is_empty());
    assert!(meaning.hyponyms.is_empty());
}

#[test]
fn every_word_of_a_meaning_is_a_word_of_the_dictionary() {
    let dictionary = dictionary();
    for text in [
        "launcher", "run", "good", "big", "device", "new york", "galore",
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

#[test]
fn word_class_has_a_name_for_the_user_interface() {
    let names: Vec<String> = [
        WordClass::Noun,
        WordClass::Verb,
        WordClass::Adjective,
        WordClass::Adverb,
    ]
    .iter()
    .map(|class| class.to_string())
    .collect();

    assert_eq!(names, ["noun", "verb", "adjective", "adverb"]);
}
