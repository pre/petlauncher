//! The Pango markup of the text of a meaning, with the links to other words.
//! Plain logic: the widgets only show the markup.

use std::fmt::Write;

use gtk::glib::markup_escape_text;
use petlauncher_dict::Meaning;

/// The start of the URI of a link to a word.
const WORD_LINK: &str = "word:";

/// The URI of the link that shows all the related words of a meaning.
const MORE_LINK: &str = "more:";

/// A meaning shows this number of more general, more specific and "see
/// also" words until the owner clicks "more". Some meanings have more than
/// 50 more specific words. The synonyms and the opposites show in full.
pub const RELATED_WORDS: usize = 4;

/// The start of a text in which a line does not break. A no-break space
/// would do the same, but it would also go to the clipboard.
const NO_BREAK: &str = "<span allow_breaks=\"false\">";

/// The space between two lists of words on one line.
const LIST_SPACE: &str = "     ";

/// The text before a list of words. The value is the alpha of the theme
/// color, so the text is quiet in the light and in the dark style.
const QUIET: &str = "fgalpha=\"62%\"";

/// A small empty line between the parts of a meaning. A label has no
/// spacing between its paragraphs.
const GAP: &str = "\n<span size=\"3pt\">\n</span>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// Open this word in the window.
    Word(String),
    /// Show all the related words of the meaning.
    More,
}

pub fn parse_link(uri: &str) -> Option<Link> {
    if let Some(word) = uri.strip_prefix(WORD_LINK) {
        return Some(Link::Word(word.to_owned()));
    }
    (uri == MORE_LINK).then_some(Link::More)
}

/// The markup of one meaning: the definition, then the examples, then two
/// quiet lines of links: the synonyms and the opposites, then the more
/// general, the more specific and the "see also" words. `headword` is the
/// word of the entry: a relation that points to it is not shown, because it
/// would be a link to the same page. With `expanded` false, the second line
/// shows at most `RELATED_WORDS` words of each list and a link to the rest.
pub fn meaning(meaning: &Meaning, headword: &str, expanded: bool) -> String {
    let mut text = markup_escape_text(&quotes(&meaning.definition)).to_string();
    if !meaning.examples.is_empty() {
        let examples: Vec<String> = meaning
            .examples
            .iter()
            .map(|example| format!("“{}”", markup_escape_text(&quotes(example))))
            .collect();
        let _ = write!(
            text,
            "{GAP}<span style=\"italic\" fgalpha=\"80%\">{}</span>",
            examples.join("   ")
        );
    }
    let other = |words: &[String]| -> Vec<String> {
        words
            .iter()
            .filter(|word| !word.eq_ignore_ascii_case(headword))
            .cloned()
            .collect()
    };
    let thesaurus = [
        ("Synonyms", other(&meaning.synonyms)),
        ("Opposite", other(&meaning.antonyms)),
    ];
    let related = [
        ("More general", other(&meaning.hypernyms)),
        ("More specific", other(&meaning.hyponyms)),
        ("See also", other(&meaning.see_also)),
    ];
    let mut hidden = 0;
    let mut related_lists = Vec::new();
    for (name, words) in related.iter().filter(|(_, words)| !words.is_empty()) {
        // A link "1 more" would take the place of the word.
        let shown = if expanded || words.len() <= RELATED_WORDS + 1 {
            words.len()
        } else {
            RELATED_WORDS
        };
        hidden += words.len() - shown;
        related_lists.push(named_list(name, &words[..shown]));
    }
    if hidden > 0 {
        related_lists.push(format!("<a href=\"{MORE_LINK}\">{hidden} more</a>"));
    }
    let thesaurus_lists: Vec<String> = thesaurus
        .iter()
        .filter(|(_, words)| !words.is_empty())
        .map(|(name, words)| named_list(name, words))
        .collect();
    let lines: Vec<String> = [thesaurus_lists, related_lists]
        .into_iter()
        .filter(|lists| !lists.is_empty())
        .map(|lists| lists.join(LIST_SPACE))
        .collect();
    if !lines.is_empty() {
        let _ = write!(
            text,
            "{GAP}<span size=\"small\">{}</span>",
            lines.join("\n")
        );
    }
    text
}

/// The name and the first word stay on one line, so a line does not end
/// with "See" or with "Synonyms".
fn named_list(name: &str, words: &[String]) -> String {
    let (first, rest) = words.split_first().expect("a list with words");
    let mut text = format!(
        "{NO_BREAK}<span {QUIET}>{name}</span>  {}</span>",
        word_links(std::slice::from_ref(first), "")
    );
    if !rest.is_empty() {
        text.push_str(", ");
        text.push_str(&word_links(rest, ", "));
    }
    text
}

/// WordNet writes a quotation inside a definition as `good'. This gives
/// ‘good’.
fn quotes(text: &str) -> String {
    let mut open = false;
    text.chars()
        .map(|character| match character {
            '`' => {
                open = true;
                '‘'
            }
            '\'' if open => {
                open = false;
                '’'
            }
            other => other,
        })
        .collect()
}

/// Links to the words, with `separator` between them. A line does not
/// break inside a word of two or more parts, such as "take to the woods".
pub fn word_links(words: &[String], separator: &str) -> String {
    words
        .iter()
        .map(|word| {
            let word = markup_escape_text(word);
            format!("{NO_BREAK}<a href=\"{WORD_LINK}{word}\">{word}</a></span>")
        })
        .collect::<Vec<_>>()
        .join(separator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary_window::source::fake;
    use petlauncher_dict::WordClass;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| (*word).to_owned()).collect()
    }

    fn launcher() -> Meaning {
        let mut meaning = fake::meaning(
            WordClass::Noun,
            "armament in the form of a device capable of launching a rocket",
        );
        meaning.synonyms = words(&["rocket launcher"]);
        meaning.hypernyms = words(&["armament"]);
        meaning.hyponyms = words(&["bazooka"]);
        meaning
    }

    /// The markup must parse. A GTK label reads the `<a>` elements itself
    /// and gives the rest to Pango, so this changes them to `<span>`.
    fn plain_text(markup: &str) -> String {
        let mut pango = String::new();
        let mut rest = markup;
        while let Some(start) = rest.find("<a href=\"") {
            pango.push_str(&rest[..start]);
            let end = start + rest[start..].find("\">").expect("the end of the tag");
            pango.push_str("<span>");
            rest = &rest[end + 2..];
        }
        pango.push_str(rest);
        let pango = pango.replace("</a>", "</span>");
        let (_, text, _) = gtk::pango::parse_markup(&pango, '\0').expect("valid markup");
        text.to_string()
    }

    #[test]
    fn a_meaning_has_the_definition_and_the_synonyms_as_links() {
        let markup = meaning(&launcher(), "launcher", false);

        assert!(markup.starts_with("armament in the form"));
        assert!(markup.contains("<a href=\"word:rocket launcher\">rocket launcher</a>"));
        assert!(markup.contains("<a href=\"word:armament\">armament</a>"));
        assert!(markup.contains("<a href=\"word:bazooka\">bazooka</a>"));
        let text = plain_text(&markup);
        assert!(text.contains("Synonyms  rocket launcher"), "{text}");
        assert!(
            text.contains("More general  armament     More specific  bazooka"),
            "{text}"
        );
    }

    #[test]
    fn examples_are_in_quotation_marks() {
        let mut better = fake::meaning(WordClass::Verb, "surpass in excellence");
        better.examples = words(&["She bettered her own record", "break a record"]);

        let text = plain_text(&meaning(&better, "better", false));

        assert!(
            text.contains("“She bettered her own record”   “break a record”"),
            "{text}"
        );
    }

    #[test]
    fn a_meaning_with_only_a_definition_has_no_more_lines() {
        let text = plain_text(&meaning(
            &fake::meaning(WordClass::Adverb, "in a better way"),
            "better",
            false,
        ));

        assert_eq!(text, "in a better way");
    }

    #[test]
    fn escapes_the_text_of_the_data() {
        let mut odd = fake::meaning(WordClass::Noun, "a <b> & \"c\"");
        odd.synonyms = words(&["o'clock"]);

        let markup = meaning(&odd, "x", false);

        assert!(markup.contains("href=\"word:o&apos;clock\""), "{markup}");
        assert!(plain_text(&markup).starts_with("a <b> & \"c\""));
    }

    #[test]
    fn a_relation_to_the_headword_is_not_a_link() {
        let mut launch = fake::meaning(WordClass::Noun, "the act of launching");
        launch.hypernyms = words(&["Launch", "propulsion"]);

        let markup = meaning(&launch, "launch", false);

        assert!(!markup.contains("word:Launch"));
        assert!(markup.contains("word:propulsion"));
    }

    #[test]
    fn a_long_relation_shows_some_words_and_a_link_to_the_rest() {
        let mut better = fake::meaning(WordClass::Verb, "to make better");
        better.hyponyms = (1..=10).map(|n| format!("word{n}")).collect();
        better.see_also = words(&["a", "b", "c", "d", "e", "f"]);

        let short = meaning(&better, "better", false);
        let long = meaning(&better, "better", true);

        assert!(short.contains("word:word4"));
        assert!(!short.contains("word:word5"));
        assert!(!short.contains("word:e"));
        assert!(
            short.ends_with("<a href=\"more:\">8 more</a></span>"),
            "{short}"
        );
        assert!(long.contains("word:word10"));
        assert!(long.contains("word:f"));
        assert!(!long.contains("more:"));
    }

    #[test]
    fn a_list_with_one_word_more_than_the_limit_shows_in_full() {
        let mut run = fake::meaning(WordClass::Verb, "stretch out over a distance");
        run.hyponyms = words(&["come", "radiate", "ray", "go far", "go deep"]);

        let markup = meaning(&run, "run", false);

        assert!(markup.contains("word:go deep"), "{markup}");
        assert!(!markup.contains("more:"), "{markup}");
    }

    #[test]
    fn synonyms_and_opposites_show_in_full_on_the_first_line() {
        let mut big = fake::meaning(WordClass::Adjective, "above average in size");
        big.synonyms = (1..=8).map(|n| format!("synonym{n}")).collect();
        big.antonyms = words(&["small"]);
        big.hypernyms = words(&["size"]);

        let text = plain_text(&meaning(&big, "big", false));

        let lines: Vec<&str> = text.lines().collect();
        // The small gap is an empty line, so a copy has no extra spaces.
        assert_eq!(lines[1], "", "{text}");
        assert!(lines[2].starts_with("Synonyms  synonym1, "), "{text}");
        assert!(lines[2].ends_with("synonym8     Opposite  small"), "{text}");
        assert_eq!(lines[3], "More general  size");
    }

    #[test]
    fn a_line_does_not_break_after_a_list_name_or_inside_a_word() {
        let mut run = fake::meaning(WordClass::Verb, "flee");
        run.see_also = words(&["run around", "run away"]);

        let markup = meaning(&run, "run", false);

        assert!(
            markup.contains(
                "<span allow_breaks=\"false\"><span fgalpha=\"62%\">See also</span>  \
                 <span allow_breaks=\"false\"><a href=\"word:run around\">run around</a></span></span>, \
                 <span allow_breaks=\"false\"><a href=\"word:run away\">"
            ),
            "{markup}"
        );
    }

    #[test]
    fn the_quotation_marks_of_wordnet_become_typographic() {
        let better = fake::meaning(WordClass::Adjective, "(comparative of `good') superior");

        let text = plain_text(&meaning(&better, "better", false));

        assert_eq!(text, "(comparative of ‘good’) superior");
        assert_eq!(quotes("don't `x'"), "don't ‘x’");
    }

    #[test]
    fn parses_the_links_of_the_markup() {
        assert_eq!(
            parse_link("word:launch pad"),
            Some(Link::Word("launch pad".into()))
        );
        assert_eq!(parse_link("more:"), Some(Link::More));
        assert_eq!(parse_link("https://example.com"), None);
    }

    #[test]
    fn word_links_have_the_separator_between_them() {
        let text = plain_text(&word_links(&words(&["launch", "launcher"]), " · "));

        assert_eq!(text, "launch · launcher");
    }
}
