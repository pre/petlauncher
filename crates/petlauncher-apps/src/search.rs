//! Match and rank. `docs/applications.md` gives the rule.

use std::path::Path;

use unicode_normalization::char::{decompose_canonical, is_combining_mark};

use crate::entry::{DesktopEntry, EntryKind};
use crate::exec;

/// Programs that start a different program. Their name tells nothing about
/// the entry. This is the list of GLib.
const WRAPPERS: &[&str] = &[
    "bash", "env", "flatpak", "gjs", "pkexec", "python", "python2", "python3", "sh", "wine",
    "wine64",
];

/// Text with case and accents removed and with only letters and digits.
/// `word_starts` holds the byte position of each word in `joined`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Text {
    pub(crate) joined: String,
    pub(crate) word_starts: Vec<usize>,
}

/// A word starts after a character that is not a letter or digit, and at a
/// capital letter after a small letter (`LibreOffice`).
pub(crate) fn normalize(text: &str) -> Text {
    let mut joined = String::with_capacity(text.len());
    let mut word_starts = Vec::new();
    let mut previous: Option<char> = None;
    for c in text.chars().filter(|&c| !is_combining_mark(c)) {
        if c.is_alphanumeric() {
            let starts_word = previous
                .is_none_or(|p| !p.is_alphanumeric() || (p.is_lowercase() && c.is_uppercase()));
            if starts_word {
                word_starts.push(joined.len());
            }
            for lower in c.to_lowercase() {
                decompose_canonical(lower, |part| {
                    if !is_combining_mark(part) {
                        joined.push(part);
                    }
                });
            }
        }
        previous = Some(c);
    }
    Text {
        joined,
        word_starts,
    }
}

impl Text {
    /// The text from each word start to the end.
    fn tails(&self) -> impl Iterator<Item = &str> {
        self.word_starts.iter().map(|&start| &self.joined[start..])
    }

    fn has_word_that_starts_with(&self, text: &str) -> bool {
        self.tails().any(|tail| tail.starts_with(text))
    }

    /// Text inside one word only: `set` does not match `Mouse & Touchpad`.
    fn has_word_that_contains(&self, text: &str) -> bool {
        self.words().any(|word| word.contains(text))
    }

    fn words(&self) -> impl Iterator<Item = &str> {
        let ends = self
            .word_starts
            .iter()
            .skip(1)
            .copied()
            .chain([self.joined.len()]);
        self.word_starts
            .iter()
            .zip(ends)
            .map(|(&start, end)| &self.joined[start..end])
    }
}

/// The match quality, worst first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Tier {
    TypingError,
    OtherSubstring,
    NameSubstring,
    GenericName,
    Keyword,
    Executable,
    Initials,
    AllWords,
    WordStart,
    NameStart,
    Exact,
}

impl Tier {
    pub(crate) fn points(self) -> u32 {
        match self {
            Tier::Exact => 1000,
            Tier::NameStart => 900,
            Tier::WordStart => 850,
            Tier::AllWords => 820,
            Tier::Initials => 800,
            Tier::Executable => 750,
            Tier::Keyword => 700,
            Tier::GenericName => 650,
            Tier::NameSubstring => 500,
            Tier::OtherSubstring => 400,
            Tier::TypingError => 200,
        }
    }
}

/// The normalized texts of one entry that the search compares.
#[derive(Debug, Clone)]
pub(crate) struct SearchKey {
    name: Text,
    initials: String,
    generic_name: Option<Text>,
    keywords: Vec<Text>,
    program: Option<String>,
}

impl SearchKey {
    pub(crate) fn new(entry: &DesktopEntry) -> SearchKey {
        let name = normalize(&entry.name);
        let initials = name
            .words()
            .filter_map(|word| word.chars().next())
            .collect();
        // All settings pages have the program of the settings application.
        let program = match entry.kind {
            EntryKind::SettingsPage => None,
            EntryKind::Application => exec::program_args(&entry.exec)
                .first()
                .and_then(|program| Path::new(program).file_name()?.to_str())
                .filter(|program| !WRAPPERS.contains(program))
                .map(|program| normalize(program).joined),
        };
        SearchKey {
            name,
            initials,
            generic_name: entry.generic_name.as_deref().map(normalize),
            keywords: entry
                .keywords
                .iter()
                .map(|keyword| normalize(keyword))
                .collect(),
            program,
        }
    }

    fn others(&self) -> impl Iterator<Item = &Text> {
        self.keywords.iter().chain(&self.generic_name)
    }

    /// The name without case, accents and punctuation, for the order of
    /// results with equal scores.
    pub(crate) fn sort_name(&self) -> &str {
        &self.name.joined
    }
}

/// A normalized query. `None` when the text has no letters or digits.
#[derive(Debug, Clone)]
pub(crate) struct Query {
    text: Text,
    chars: Vec<char>,
}

impl Query {
    pub(crate) fn new(text: &str) -> Option<Query> {
        let text = normalize(text);
        let chars: Vec<char> = text.joined.chars().collect();
        (!chars.is_empty()).then_some(Query { text, chars })
    }
}

/// Text inside a word matches only from this length, so that two letters do
/// not find most of the entries.
const SUBSTRING_LENGTH: usize = 3;

/// A typing error is accepted only from this length.
const TYPING_ERROR_LENGTH: usize = 4;

/// The best tier in which `query` matches `key`.
pub(crate) fn tier(key: &SearchKey, query: &Query) -> Option<Tier> {
    let text = query.text.joined.as_str();
    let name = &key.name;
    let substring = query.chars.len() >= SUBSTRING_LENGTH;

    let tier = if name.joined == text {
        Tier::Exact
    } else if name.joined.starts_with(text) {
        Tier::NameStart
    } else if name.has_word_that_starts_with(text) {
        Tier::WordStart
    } else if query.text.word_starts.len() > 1
        && query
            .text
            .words()
            .all(|word| name.has_word_that_starts_with(word))
    {
        Tier::AllWords
    } else if query.chars.len() > 1 && key.initials.starts_with(text) {
        Tier::Initials
    } else if key
        .program
        .as_ref()
        .is_some_and(|program| program.starts_with(text))
    {
        Tier::Executable
    } else if key
        .keywords
        .iter()
        .any(|keyword| keyword.has_word_that_starts_with(text))
    {
        Tier::Keyword
    } else if key
        .generic_name
        .as_ref()
        .is_some_and(|generic| generic.has_word_that_starts_with(text))
    {
        Tier::GenericName
    } else if substring && name.has_word_that_contains(text) {
        Tier::NameSubstring
    } else if substring && key.others().any(|other| other.has_word_that_contains(text)) {
        Tier::OtherSubstring
    } else if query.chars.len() >= TYPING_ERROR_LENGTH
        && name
            .tails()
            .any(|tail| starts_with_one_typing_error(tail, &query.chars))
    {
        Tier::TypingError
    } else {
        return None;
    };
    Some(tier)
}

/// `true` when a start of `text` differs from `query` by one substitution,
/// insertion, deletion or transposition of two adjacent characters. The
/// first character must be correct.
fn starts_with_one_typing_error(text: &str, query: &[char]) -> bool {
    let text: Vec<char> = text.chars().take(query.len() + 1).collect();
    if text.first() != query.first() {
        return false;
    }
    // distance[i][j]: the optimal string alignment distance between
    // query[..i] and text[..j].
    let (n, m) = (query.len(), text.len());
    let mut distance = vec![vec![0; m + 1]; n + 1];
    for (i, row) in distance.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in distance[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let substitution = distance[i - 1][j - 1] + usize::from(query[i - 1] != text[j - 1]);
            let mut best = substitution
                .min(distance[i - 1][j] + 1)
                .min(distance[i][j - 1] + 1);
            if i > 1 && j > 1 && query[i - 1] == text[j - 2] && query[i - 2] == text[j - 1] {
                best = best.min(distance[i - 2][j - 2] + 1);
            }
            distance[i][j] = best;
        }
    }
    (n.saturating_sub(1)..=m).any(|j| distance[n][j] <= 1)
}

/// Points for the number of starts: 25 for each doubling, at most 200.
pub(crate) fn usage_bonus(starts: u32) -> u32 {
    let bonus = 25.0 * (f64::from(starts) + 1.0).log2();
    (bonus as u32).min(200)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EntryKind;

    fn tier_of(entry: &DesktopEntry, text: &str) -> Option<Tier> {
        tier(&SearchKey::new(entry), &Query::new(text).unwrap())
    }

    fn app(name: &str) -> DesktopEntry {
        DesktopEntry::for_test(name, &["unrelated-program"])
    }

    #[test]
    fn removes_case_accents_and_punctuation_and_marks_the_words() {
        let text = normalize("Visual Studio Code");
        let wifi = normalize("Wi-Fi");

        assert_eq!(text.joined, "visualstudiocode");
        assert_eq!(text.word_starts, [0, 6, 12]);
        assert_eq!(wifi.joined, "wifi");
        assert_eq!(wifi.word_starts, [0, 2]);
        assert_eq!(normalize("PÄIVYRI Éditeur").joined, "paivyriediteur");
    }

    #[test]
    fn reads_decomposed_accents_as_part_of_the_word() {
        let text = normalize("Pa\u{308}ivyri");

        assert_eq!(text.joined, "paivyri");
        assert_eq!(text.word_starts, [0]);
    }

    #[test]
    fn starts_a_word_at_a_capital_letter_after_a_small_letter() {
        assert_eq!(normalize("LibreOffice Writer").word_starts, [0, 5, 11]);
        assert_eq!(normalize("GNOME Logs").word_starts, [0, 5]);
    }

    #[test]
    fn gives_no_query_for_text_without_letters_or_digits() {
        for text in ["", "   ", "-+ ."] {
            assert!(Query::new(text).is_none(), "{text:?}");
        }
    }

    #[test]
    fn finds_an_exact_name_with_case_and_punctuation_ignored() {
        assert_eq!(tier_of(&app("Files"), "files"), Some(Tier::Exact));
        assert_eq!(tier_of(&app("Wi-Fi"), "wifi"), Some(Tier::Exact));
    }

    #[test]
    fn finds_a_name_that_starts_with_the_text() {
        assert_eq!(tier_of(&app("Terminal"), "TERM"), Some(Tier::NameStart));
        assert_eq!(
            tier_of(&app("Text Editor"), "text ed"),
            Some(Tier::NameStart)
        );
    }

    #[test]
    fn finds_a_word_of_the_name_that_starts_with_the_text() {
        let code = app("Visual Studio Code");

        assert_eq!(tier_of(&code, "code"), Some(Tier::WordStart));
        assert_eq!(tier_of(&code, "studio co"), Some(Tier::WordStart));
    }

    #[test]
    fn finds_query_words_that_start_name_words_in_any_order() {
        let code = app("Visual Studio Code");

        assert_eq!(tier_of(&code, "vis code"), Some(Tier::AllWords));
        assert_eq!(tier_of(&code, "code vis"), Some(Tier::AllWords));
    }

    #[test]
    fn finds_the_initials_of_the_name() {
        assert_eq!(tier_of(&app("Google Chrome"), "gc"), Some(Tier::Initials));
        assert_eq!(
            tier_of(&app("Visual Studio Code"), "vsc"),
            Some(Tier::Initials)
        );
    }

    #[test]
    fn finds_the_program_name_of_an_application() {
        let files = DesktopEntry::for_test("Files", &["nautilus", "--new-window", "%U"]);
        let chatgpt = DesktopEntry::for_test("ChatGPT", &["env", "A=1", "/usr/bin/chatgpt"]);

        assert_eq!(tier_of(&files, "nautilus"), Some(Tier::Executable));
        assert_eq!(tier_of(&chatgpt, "chatg"), Some(Tier::NameStart));
        assert_eq!(tier_of(&chatgpt, "/usr/bin"), None);
    }

    #[test]
    fn ignores_the_program_name_of_a_wrapper_and_of_a_settings_page() {
        let flatpak =
            DesktopEntry::for_test("Kukka", &["/usr/bin/flatpak", "run", "dev.kukka.Kukka"]);
        let mut page = DesktopEntry::for_test("Wi-Fi", &["gnome-control-center", "wifi"]);
        page.kind = EntryKind::SettingsPage;

        assert_eq!(tier_of(&flatpak, "flatpak"), None);
        assert_eq!(tier_of(&page, "gnome"), None);
    }

    #[test]
    fn finds_a_word_of_a_keyword_that_starts_with_the_text() {
        let mut terminal = app("Terminal");
        terminal.keywords = vec!["shell".into(), "Command Line".into()];

        assert_eq!(tier_of(&terminal, "shel"), Some(Tier::Keyword));
        assert_eq!(tier_of(&terminal, "line"), Some(Tier::Keyword));
    }

    #[test]
    fn finds_a_word_of_the_generic_name_that_starts_with_the_text() {
        let mut chrome = app("Google Chrome");
        chrome.generic_name = Some("Web Browser".into());

        assert_eq!(tier_of(&chrome, "brow"), Some(Tier::GenericName));
    }

    #[test]
    fn finds_text_inside_the_name_from_three_characters() {
        let editor = app("Text Editor");

        assert_eq!(tier_of(&editor, "ditor"), Some(Tier::NameSubstring));
        assert_eq!(tier_of(&editor, "it"), None);
    }

    #[test]
    fn finds_text_inside_one_word_only() {
        let mut touchpad = app("Mouse & Touchpad");
        touchpad.keywords = vec!["Trackpad Pointer".into()];

        assert_eq!(tier_of(&touchpad, "set"), None);
        assert_eq!(tier_of(&touchpad, "dpoint"), None);
        assert_eq!(tier_of(&touchpad, "ouch"), Some(Tier::NameSubstring));
    }

    #[test]
    fn finds_text_inside_a_keyword_or_the_generic_name_from_three_characters() {
        let mut chrome = app("Google Chrome");
        chrome.generic_name = Some("Web Browser".into());
        chrome.keywords = vec!["internet".into()];

        assert_eq!(tier_of(&chrome, "rowser"), Some(Tier::OtherSubstring));
        assert_eq!(tier_of(&chrome, "tern"), Some(Tier::OtherSubstring));
        assert_eq!(tier_of(&chrome, "er"), None);
    }

    #[test]
    fn finds_a_name_with_one_typing_error_from_four_characters() {
        let terminal = app("Terminal");

        assert_eq!(tier_of(&terminal, "termnial"), Some(Tier::TypingError));
        assert_eq!(tier_of(&terminal, "temrinal"), Some(Tier::TypingError));
        assert_eq!(tier_of(&terminal, "teminal"), Some(Tier::TypingError));
        assert_eq!(tier_of(&terminal, "terrminal"), Some(Tier::TypingError));
        assert_eq!(tier_of(&terminal, "tarm"), Some(Tier::TypingError));
        assert_eq!(tier_of(&app("Firefox"), "firefx"), Some(Tier::TypingError));
        assert_eq!(
            tier_of(&app("Microsoft Edge"), "egde"),
            Some(Tier::TypingError)
        );
    }

    #[test]
    fn does_not_accept_a_typing_error_that_adds_noise() {
        let terminal = app("Terminal");

        assert_eq!(
            tier_of(&terminal, "tem"),
            None,
            "fewer than four characters"
        );
        assert_eq!(
            tier_of(&terminal, "germ"),
            None,
            "first character is different"
        );
        assert_eq!(tier_of(&terminal, "tremnial"), None, "two errors");
    }

    #[test]
    fn gives_no_tier_when_nothing_matches() {
        assert_eq!(tier_of(&app("Terminal"), "zzz"), None);
    }

    #[test]
    fn orders_the_tiers_by_their_points() {
        let tiers = [
            Tier::TypingError,
            Tier::OtherSubstring,
            Tier::NameSubstring,
            Tier::GenericName,
            Tier::Keyword,
            Tier::Executable,
            Tier::Initials,
            Tier::AllWords,
            Tier::WordStart,
            Tier::NameStart,
            Tier::Exact,
        ];

        let points: Vec<u32> = tiers.iter().map(|tier| tier.points()).collect();

        assert_eq!(
            points,
            [200, 400, 500, 650, 700, 750, 800, 820, 850, 900, 1000]
        );
    }

    #[test]
    fn gives_25_points_for_each_doubling_of_the_starts_up_to_200() {
        let bonus: Vec<u32> = [0, 1, 3, 7, 15, 255, 100_000].map(usage_bonus).to_vec();

        assert_eq!(bonus, [0, 25, 50, 75, 100, 200, 200]);
    }
}
