//! The WordNet morphology: finds the base form of an inflected word with the
//! exception files and the suffix rules of each word class.

use std::collections::HashMap;
use std::path::Path;

use crate::index::read_text;
use crate::{Error, WordClass};

/// The exception lists `noun.exc`, `verb.exc`, `adj.exc` and `adv.exc`: an
/// irregular form and its base forms.
pub(crate) struct Exceptions([HashMap<String, Vec<String>>; 4]);

impl Exceptions {
    pub(crate) fn read(dir: &Path) -> Result<Exceptions, Error> {
        let [noun, verb, adjective, adverb] = WordClass::ALL
            .map(|class| read_text(&dir.join(format!("{}.exc", class.file_suffix()))));
        Ok(Exceptions::parse([&noun?, &verb?, &adjective?, &adverb?]))
    }

    /// Each line is an inflected form and one or more base forms.
    fn parse(texts: [&str; 4]) -> Exceptions {
        Exceptions(texts.map(|text| {
            text.lines()
                .filter_map(|line| {
                    let mut fields = line.split_ascii_whitespace();
                    let form = fields.next()?.to_lowercase();
                    let bases: Vec<String> = fields.map(str::to_lowercase).collect();
                    (!bases.is_empty()).then_some((form, bases))
                })
                .collect()
        }))
    }

    fn get(&self, class: WordClass, word: &str) -> Option<&[String]> {
        self.0[class as usize].get(word).map(Vec::as_slice)
    }
}

/// The suffix rules of the WordNet morphology: an inflected word that ends in
/// the first text can have a base form that ends in the second text.
const NOUN_RULES: &[(&str, &str)] = &[
    ("s", ""),
    ("ses", "s"),
    ("xes", "x"),
    ("zes", "z"),
    ("ches", "ch"),
    ("shes", "sh"),
    ("men", "man"),
    ("ies", "y"),
];
const VERB_RULES: &[(&str, &str)] = &[
    ("s", ""),
    ("ies", "y"),
    ("es", "e"),
    ("es", ""),
    ("ed", "e"),
    ("ed", ""),
    ("ing", "e"),
    ("ing", ""),
];
const ADJECTIVE_RULES: &[(&str, &str)] = &[("er", ""), ("est", ""), ("er", "e"), ("est", "e")];

/// The base forms of `word` in `class`, without `word` itself. `is_word`
/// tells if a form is a word of `class`.
///
/// A word in the exception list gets the base forms of the list only. A
/// different word gets the first rule that gives a word. A collocation gets
/// the base form of each part.
pub(crate) fn base_forms(
    word: &str,
    class: WordClass,
    exceptions: &Exceptions,
    is_word: &dyn Fn(&str) -> bool,
) -> Vec<String> {
    if let Some(bases) = exceptions.get(class, word) {
        return bases
            .iter()
            .filter(|base| *base != word && is_word(base))
            .cloned()
            .collect();
    }
    if word.contains('_') {
        let parts: Vec<String> = word
            .split('_')
            .map(|part| {
                base_of_part(part, class, exceptions, is_word).unwrap_or_else(|| part.to_owned())
            })
            .collect();
        let base = parts.join("_");
        return if base != word && is_word(&base) {
            vec![base]
        } else {
            Vec::new()
        };
    }
    by_rules(word, class, is_word).into_iter().collect()
}

fn base_of_part(
    part: &str,
    class: WordClass,
    exceptions: &Exceptions,
    is_word: &dyn Fn(&str) -> bool,
) -> Option<String> {
    match exceptions.get(class, part) {
        Some(bases) => bases.first().cloned(),
        None => by_rules(part, class, is_word),
    }
}

fn by_rules(word: &str, class: WordClass, is_word: &dyn Fn(&str) -> bool) -> Option<String> {
    let rules = match class {
        WordClass::Noun => NOUN_RULES,
        WordClass::Verb => VERB_RULES,
        WordClass::Adjective => ADJECTIVE_RULES,
        WordClass::Adverb => return None,
    };
    if class == WordClass::Noun {
        if let Some(stem) = word.strip_suffix("ful") {
            let with_ful = |form: &str| is_word(&format!("{form}ful"));
            return first_rule(stem, rules, &with_ful).map(|form| form + "ful");
        }
        if word.ends_with("ss") || word.len() <= 2 {
            return None;
        }
    }
    first_rule(word, rules, is_word)
}

fn first_rule(
    word: &str,
    rules: &[(&str, &str)],
    is_word: &dyn Fn(&str) -> bool,
) -> Option<String> {
    rules.iter().find_map(|(suffix, ending)| {
        let form = format!("{}{ending}", word.strip_suffix(suffix)?);
        (form != word && is_word(&form)).then_some(form)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exceptions() -> Exceptions {
        Exceptions::parse([
            "axes ax axis\nmen man\n",
            "hopping hop\nran run\n",
            "better good well\nbigger big\n",
            "better well\n",
        ])
    }

    fn forms(word: &str, class: WordClass, words: &[&str]) -> Vec<String> {
        let is_word = |form: &str| words.contains(&form);
        base_forms(word, class, &exceptions(), &is_word)
    }

    #[test]
    fn plural_noun_gives_the_singular() {
        assert_eq!(
            forms("launchers", WordClass::Noun, &["launcher"]),
            ["launcher"]
        );
        assert_eq!(forms("buses", WordClass::Noun, &["bus"]), ["bus"]);
        assert_eq!(forms("boxes", WordClass::Noun, &["box"]), ["box"]);
        assert_eq!(forms("matches", WordClass::Noun, &["match"]), ["match"]);
        assert_eq!(forms("dishes", WordClass::Noun, &["dish"]), ["dish"]);
        assert_eq!(forms("firemen", WordClass::Noun, &["fireman"]), ["fireman"]);
        assert_eq!(forms("cities", WordClass::Noun, &["city"]), ["city"]);
    }

    #[test]
    fn form_that_is_not_a_word_is_not_a_base_form() {
        assert!(forms("launchers", WordClass::Noun, &[]).is_empty());
    }

    #[test]
    fn noun_that_ends_in_ss_or_is_short_has_no_rule() {
        assert!(forms("boss", WordClass::Noun, &["bos"]).is_empty());
        assert!(forms("as", WordClass::Noun, &["a"]).is_empty());
    }

    #[test]
    fn noun_that_ends_in_ful_keeps_the_ful() {
        assert_eq!(forms("boxesful", WordClass::Noun, &["boxful"]), ["boxful"]);
    }

    #[test]
    fn exception_list_gives_irregular_base_forms() {
        assert_eq!(forms("ran", WordClass::Verb, &["run"]), ["run"]);
        assert_eq!(
            forms("axes", WordClass::Noun, &["ax", "axis"]),
            ["ax", "axis"]
        );
        assert_eq!(
            forms("better", WordClass::Adjective, &["good", "well", "better"]),
            ["good", "well"]
        );
        assert_eq!(forms("better", WordClass::Adverb, &["well"]), ["well"]);
    }

    #[test]
    fn exception_base_form_that_is_not_a_word_is_skipped() {
        assert_eq!(forms("axes", WordClass::Noun, &["axis"]), ["axis"]);
    }

    #[test]
    fn word_in_the_exception_list_gets_no_rule() {
        assert!(forms("men", WordClass::Noun, &["me"]).is_empty());
    }

    #[test]
    fn verb_rules_give_the_first_form_that_is_a_word() {
        assert_eq!(forms("hoped", WordClass::Verb, &["hope", "hop"]), ["hope"]);
        assert_eq!(forms("wanted", WordClass::Verb, &["want"]), ["want"]);
        assert_eq!(forms("hoping", WordClass::Verb, &["hope", "hop"]), ["hope"]);
        assert_eq!(forms("tries", WordClass::Verb, &["try"]), ["try"]);
        assert_eq!(forms("hopping", WordClass::Verb, &["hop"]), ["hop"]);
    }

    #[test]
    fn adjective_rules_remove_er_and_est() {
        assert_eq!(forms("riper", WordClass::Adjective, &["ripe"]), ["ripe"]);
        assert_eq!(
            forms("smallest", WordClass::Adjective, &["small"]),
            ["small"]
        );
        assert_eq!(forms("bigger", WordClass::Adjective, &["big"]), ["big"]);
    }

    #[test]
    fn adverb_has_exceptions_only() {
        assert!(forms("faster", WordClass::Adverb, &["fast"]).is_empty());
    }

    #[test]
    fn collocation_gets_the_base_form_of_each_part() {
        let words = ["look", "up", "look_up"];

        assert_eq!(forms("looked_up", WordClass::Verb, &words), ["look_up"]);
        assert_eq!(
            forms("ran_away", WordClass::Verb, &["run", "run_away"]),
            ["run_away"]
        );
        assert!(forms("looked_up", WordClass::Verb, &["look"]).is_empty());
    }
}
