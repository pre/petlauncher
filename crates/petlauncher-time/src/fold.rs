use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Gives the key that city names are compared with: lower case, no accents,
/// no `.` or `'`, a space for `-` and `_`, one space between words, and "st"
/// for the word "saint".
pub(crate) fn fold(name: &str) -> String {
    let mut key = String::with_capacity(name.len());
    for c in name.nfd().flat_map(char::to_lowercase) {
        match c {
            '-' | '_' => key.push(' '),
            '.' | '\'' | '‘' | '’' => {}
            // Letters that have no accent to remove.
            'ø' => key.push('o'),
            'æ' => key.push_str("ae"),
            'œ' => key.push_str("oe"),
            'ß' => key.push_str("ss"),
            'ð' | 'đ' => key.push('d'),
            'ł' => key.push('l'),
            'þ' => key.push_str("th"),
            'ı' => key.push('i'),
            c if is_combining_mark(c) => {}
            c => key.push(c),
        }
    }
    key.split_whitespace()
        .map(|word| if word == "saint" { "st" } else { word })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn fold_removes_case_accents_and_punctuation() {
        // Arrange
        let names = [
            ("Godthåb", "godthab"),
            ("Søndre Strømfjord", "sondre stromfjord"),
            ("Łódź", "lodz"),
            ("St. John's", "st johns"),
            ("Ust-Nera", "ust nera"),
            ("Buenos_Aires", "buenos aires"),
            ("  New   YORK ", "new york"),
            ("Ærøskøbing", "aeroskobing"),
            ("Reykjavík", "reykjavik"),
            ("Þórshöfn", "thorshofn"),
            ("Reyðarfjörður", "reydarfjordur"),
            ("Diyarbakır", "diyarbakir"),
            ("Œuilly", "oeuilly"),
            ("Saint John's", "st johns"),
            ("Saint-Pierre", "st pierre"),
            ("Saintes", "saintes"),
        ];

        for (name, key) in names {
            // Act
            let folded = fold(name);

            // Assert
            assert_eq!(folded, key, "{name}");
        }
    }
}
