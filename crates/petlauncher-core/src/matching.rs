//! The text match for the built-in rows: system commands and settings pages.

/// Gives the rank of a match of `query` with a title and its keywords, or
/// `None` when they do not match. A lower rank is a better match.
///
/// The comparison ignores letter case and all characters that are not
/// letters or digits, so `wifi` matches `Wi-Fi`.
///
/// - 0: the title starts with the query.
/// - 1: a word of the title starts with the query.
/// - 2: a keyword, or a word of a keyword, starts with the query.
pub fn match_rank<K: AsRef<str>>(query: &str, title: &str, keywords: &[K]) -> Option<u8> {
    let query = compact(query);
    if query.is_empty() {
        return None;
    }
    if compact(title).starts_with(&query) {
        return Some(0);
    }
    if words(title).any(|word| word.starts_with(&query)) {
        return Some(1);
    }
    let keyword_matches = keywords.iter().any(|keyword| {
        let keyword = keyword.as_ref();
        compact(keyword).starts_with(&query) || words(keyword).any(|word| word.starts_with(&query))
    });
    keyword_matches.then_some(2)
}

fn compact(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(compact)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: [&str; 0] = [];

    #[test]
    fn a_start_of_the_title_is_the_best_match() {
        assert_eq!(match_rank("pow", "Power off", &NONE), Some(0));
        assert_eq!(match_rank("POWER OF", "Power off", &NONE), Some(0));
    }

    #[test]
    fn a_start_of_a_word_of_the_title_matches() {
        assert_eq!(match_rank("off", "Power off", &NONE), Some(1));
    }

    #[test]
    fn the_match_ignores_punctuation() {
        assert_eq!(match_rank("wifi", "Wi-Fi", &NONE), Some(0));
    }

    #[test]
    fn a_keyword_matches_after_the_title() {
        assert_eq!(match_rank("reboot", "Restart", &["reboot"]), Some(2));
        assert_eq!(match_rank("down", "Power off", &["shut down"]), Some(2));
    }

    #[test]
    fn text_in_the_middle_of_a_word_does_not_match() {
        assert_eq!(match_rank("art", "Restart", &NONE), None);
    }

    #[test]
    fn an_empty_query_matches_nothing() {
        assert_eq!(match_rank("  ", "Lock", &NONE), None);
    }
}
