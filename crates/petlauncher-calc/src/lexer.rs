//! Splits the query text into tokens.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    /// A number literal in the form that `rust_decimal` reads, for example `0.5` or `1.5e3`.
    Number(String),
    /// A function, constant, keyword or unit name, as typed.
    Name(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Percent,
    Open,
    Close,
    Comma,
}

impl Token {
    pub(crate) fn is_binary_operator(&self) -> bool {
        matches!(
            self,
            Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Caret
        )
    }
}

/// Returns `None` when the text has a character that no calculation uses.
pub(crate) fn tokenize(text: &str) -> Option<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let token = match c {
            c if c.is_whitespace() => {
                i += 1;
                continue;
            }
            '0'..='9' | '.' => {
                let (literal, next) = number(&chars, i)?;
                i = next;
                tokens.push(Token::Number(literal));
                continue;
            }
            c if c.is_alphabetic() || c == '°' || c == '_' => {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                tokens.push(Token::Name(chars[start..i].iter().collect()));
                continue;
            }
            '*' if chars.get(i + 1) == Some(&'*') => {
                i += 1;
                Token::Caret
            }
            '+' => Token::Plus,
            '-' | '−' => Token::Minus,
            '*' | '×' => Token::Star,
            '/' | '÷' => Token::Slash,
            '^' => Token::Caret,
            '%' => Token::Percent,
            '(' => Token::Open,
            ')' => Token::Close,
            ',' => Token::Comma,
            _ => return None,
        };
        tokens.push(token);
        i += 1;
    }
    Some(tokens)
}

/// Reads `digits [. digits] [e [sign] digits]` from `start`. Returns the
/// literal and the index after it.
fn number(chars: &[char], start: usize) -> Option<(String, usize)> {
    let digits_from = |mut i: usize, text: &mut String| {
        while let Some(c) = chars.get(i).filter(|c| c.is_ascii_digit()) {
            text.push(*c);
            i += 1;
        }
        i
    };

    let mut integer = String::new();
    let mut i = digits_from(start, &mut integer);
    let mut fraction = String::new();
    if chars.get(i) == Some(&'.') {
        i = digits_from(i + 1, &mut fraction);
    }
    if integer.is_empty() && fraction.is_empty() {
        return None;
    }

    let mut literal = if integer.is_empty() {
        "0".into()
    } else {
        integer
    };
    if !fraction.is_empty() {
        literal.push('.');
        literal.push_str(&fraction);
    }

    if matches!(chars.get(i), Some('e' | 'E')) {
        let mut j = i + 1;
        let mut exponent = String::from("e");
        if let Some(sign @ ('+' | '-')) = chars.get(j) {
            exponent.push(*sign);
            j += 1;
        }
        if chars.get(j).is_some_and(char::is_ascii_digit) {
            i = digits_from(j, &mut exponent);
            literal.push_str(&exponent);
        }
    }
    Some((literal, i))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number_literal(text: &str) -> String {
        match tokenize(text).unwrap().as_slice() {
            [Token::Number(literal)] => literal.clone(),
            other => panic!("`{text}` gave {other:?}"),
        }
    }

    #[test]
    fn number_literals_get_the_form_that_decimal_reads() {
        assert_eq!(number_literal("42"), "42");
        assert_eq!(number_literal(".5"), "0.5");
        assert_eq!(number_literal("5."), "5");
        assert_eq!(number_literal("1.5E3"), "1.5e3");
        assert_eq!(number_literal("2e-3"), "2e-3");
    }

    #[test]
    fn letter_e_without_exponent_digits_is_a_name() {
        let tokens = tokenize("2e").unwrap();

        assert_eq!(
            tokens,
            vec![Token::Number("2".into()), Token::Name("e".into())]
        );
    }

    #[test]
    fn number_and_unit_without_space_are_two_tokens() {
        let tokens = tokenize("5GiB").unwrap();

        assert_eq!(
            tokens,
            vec![Token::Number("5".into()), Token::Name("GiB".into())]
        );
    }

    #[test]
    fn degree_sign_starts_a_name() {
        let tokens = tokenize("°C").unwrap();

        assert_eq!(tokens, vec![Token::Name("°C".into())]);
    }

    #[test]
    fn double_star_is_the_power_operator() {
        let tokens = tokenize("2**3").unwrap();

        assert_eq!(tokens[1], Token::Caret);
    }

    #[test]
    fn character_that_no_calculation_uses_gives_none() {
        assert_eq!(tokenize("~/a"), None);
        assert_eq!(tokenize("17:00"), None);
        assert_eq!(tokenize("."), None);
    }
}
