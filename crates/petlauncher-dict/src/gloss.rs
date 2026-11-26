//! Separates the definition and the example sentences of a WordNet gloss.
//!
//! A gloss is the definition, then the examples in quotation marks, with `;`
//! between the parts: `send into the air; "launch a rocket"`. Some glosses of
//! WordNet 3.0 do not obey this form. The rules below accept the frequent
//! errors: `:` or `,` before the first example, a missing quotation mark, and
//! the name of an author after an example.

/// Splits a gloss into the definition and the example sentences.
pub(crate) fn split(gloss: &str) -> (String, Vec<String>) {
    let mut definition: Vec<&str> = Vec::new();
    let mut examples = Vec::new();
    for part in parts(gloss) {
        if !examples.is_empty() || part.starts_with('"') {
            examples.extend(quoted(part));
        } else if let Some((text, rest)) = example_after_colon_or_comma(part) {
            definition.push(text);
            examples.extend(quoted(rest));
        } else if !definition.is_empty() && part.ends_with('"') && part.matches('"').count() == 1 {
            // An example without its first quotation mark.
            examples.extend(quoted(part));
        } else {
            definition.push(part);
        }
    }
    (definition.join("; "), examples)
}

/// The parts of the gloss between the `;` that are not in quotation marks.
fn parts(gloss: &str) -> impl Iterator<Item = &str> {
    let mut in_quotes = false;
    gloss
        .split(move |c| {
            if c == '"' {
                in_quotes = !in_quotes;
            }
            c == ';' && !in_quotes
        })
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

/// Splits `remove slowly: "an example"` before the quotation mark.
fn example_after_colon_or_comma(part: &str) -> Option<(&str, &str)> {
    let quote = part.find('"')?;
    let text = part[..quote].trim_end();
    let text = text.strip_suffix([':', ','])?;
    Some((text.trim_end(), &part[quote..]))
}

/// The examples in a part of a gloss. The text after the last quotation mark,
/// usually the name of an author, stays with the last example.
fn quoted(part: &str) -> Vec<String> {
    if !part.starts_with('"') {
        return vec![part.replace('"', "").trim().to_owned()];
    }
    let mut examples: Vec<String> = Vec::new();
    for (number, piece) in part.split('"').enumerate().skip(1) {
        let inside_quotes = number % 2 == 1;
        let piece = piece.trim();
        if inside_quotes {
            if !piece.is_empty() {
                examples.push(piece.to_owned());
            }
            continue;
        }
        let after = piece.trim_start_matches(',').trim_start();
        if let Some(last) = examples.last_mut() {
            if !after.is_empty() {
                last.push(' ');
                last.push_str(after);
            }
        }
    }
    examples
}

#[cfg(test)]
mod tests {
    use super::split;

    fn examples(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn gloss_without_examples_is_the_definition() {
        let gloss = "a thing that is made for a special purpose  ";

        let (definition, found) = split(gloss);

        assert_eq!(definition, "a thing that is made for a special purpose");
        assert!(found.is_empty());
    }

    #[test]
    fn quoted_parts_after_semicolons_are_examples() {
        let gloss = r#"send into the air; "they launched a rocket"; "launch a boat""#;

        let (definition, found) = split(gloss);

        assert_eq!(definition, "send into the air");
        assert_eq!(
            found,
            examples(&["they launched a rocket", "launch a boat"])
        );
    }

    #[test]
    fn definition_with_several_parts_keeps_its_semicolons() {
        let gloss = r#"fully; completely; "do it well""#;

        let (definition, found) = split(gloss);

        assert_eq!(definition, "fully; completely");
        assert_eq!(found, examples(&["do it well"]));
    }

    #[test]
    fn semicolon_in_an_example_does_not_split_it() {
        let gloss = r#"be shy; "I never spoke; I looked at the wall""#;

        let (definition, found) = split(gloss);

        assert_eq!(definition, "be shy");
        assert_eq!(found, examples(&["I never spoke; I looked at the wall"]));
    }

    #[test]
    fn author_after_an_example_stays_with_the_example() {
        let gloss = r#"at no time; "never again"- A. Writer; "not now""#;

        let (definition, found) = split(gloss);

        assert_eq!(definition, "at no time");
        assert_eq!(found, examples(&["never again - A. Writer", "not now"]));
    }

    #[test]
    fn examples_with_a_comma_between_them_are_two_examples() {
        let gloss = r#"completely; "well-done beef", "well-made chairs""#;

        let (_, found) = split(gloss);

        assert_eq!(found, examples(&["well-done beef", "well-made chairs"]));
    }

    #[test]
    fn example_after_a_colon_or_a_comma_is_an_example() {
        let colon = r#"remove slowly: "the river takes the bank away""#;
        let comma = r#"cause to speak, "can you make her talk""#;

        let (colon_definition, colon_examples) = split(colon);
        let (comma_definition, comma_examples) = split(comma);

        assert_eq!(colon_definition, "remove slowly");
        assert_eq!(colon_examples, examples(&["the river takes the bank away"]));
        assert_eq!(comma_definition, "cause to speak");
        assert_eq!(comma_examples, examples(&["can you make her talk"]));
    }

    #[test]
    fn quoted_words_in_a_definition_stay_in_the_definition() {
        let gloss = r#"a reward, as in "carrot and stick" for a mule"#;

        let (definition, found) = split(gloss);

        assert_eq!(
            definition,
            r#"a reward, as in "carrot and stick" for a mule"#
        );
        assert!(found.is_empty());
    }

    #[test]
    fn example_without_its_first_quotation_mark_is_an_example() {
        let gloss = r#"focus again; The man focused the beam""#;

        let (definition, found) = split(gloss);

        assert_eq!(definition, "focus again");
        assert_eq!(found, examples(&["The man focused the beam"]));
    }

    #[test]
    fn first_part_with_a_quotation_mark_at_its_end_is_the_definition() {
        let gloss = r#"a place where you can send letters""#;

        let (definition, found) = split(gloss);

        assert_eq!(definition, r#"a place where you can send letters""#);
        assert!(found.is_empty());
    }

    #[test]
    fn text_after_the_first_example_is_an_example() {
        let gloss = r#"say lightly; "drop a hint"; drop names""#;

        let (definition, found) = split(gloss);

        assert_eq!(definition, "say lightly");
        assert_eq!(found, examples(&["drop a hint", "drop names"]));
    }

    #[test]
    fn example_without_a_space_after_the_semicolon_is_an_example() {
        let gloss = r#"burst;"the bomb burst at noon""#;

        let (definition, found) = split(gloss);

        assert_eq!(definition, "burst");
        assert_eq!(found, examples(&["the bomb burst at noon"]));
    }

    #[test]
    fn gloss_with_examples_only_has_an_empty_definition() {
        let gloss = r#""an example""#;

        let (definition, found) = split(gloss);

        assert_eq!(definition, "");
        assert_eq!(found, examples(&["an example"]));
    }
}
