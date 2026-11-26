use jiff::civil::Time;

/// A time query as the owner typed it. The parse does not look at cities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Query {
    pub form: Form,
    /// The city name in lower case, for example "las vegas".
    pub city: String,
    /// True when the text can only be a time query, so an unknown city gives
    /// a "city not found" row. False for `now in <x>` and `18 in <x>`, which
    /// can be different text, for example `now in stock`.
    pub clear: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Form {
    /// `time now in <city>`, `time in <city>` or `now in <city>`.
    Now,
    /// `<time> in <city>`: the time is in the city.
    InCity(Time),
    /// `<time> here in <city>`: the time is in the owner's time zone.
    Here(Time),
}

/// Reads a time query. Gives `None` for text that is not a time query.
pub(crate) fn parse(text: &str) -> Option<Query> {
    let text = text.to_lowercase();
    let words: Vec<&str> = text.split_whitespace().collect();
    let in_position = words.iter().position(|word| *word == "in")?;
    let (before, city) = (&words[..in_position], &words[in_position + 1..]);
    if city.is_empty() {
        return None;
    }
    let (form, clear) = match before {
        ["time"] | ["time", "now"] => (Form::Now, true),
        ["now"] => (Form::Now, false),
        [time @ .., "here"] => {
            let (time, clear) = parse_time(&time.join(" "))?;
            (Form::Here(time), clear)
        }
        time => {
            let (time, clear) = parse_time(&time.join(" "))?;
            (Form::InCity(time), clear)
        }
    };
    Some(Query {
        form,
        city: city.join(" "),
        clear,
    })
}

/// Reads `18`, `18:30`, `6am`, `6 am`, `6:30pm`, `6 p.m.`, `noon` or
/// `midnight`. The flag is true when the text can only be a time: all forms
/// except a bare hour such as `18`.
fn parse_time(text: &str) -> Option<(Time, bool)> {
    match text {
        "noon" => return Some((Time::constant(12, 0, 0, 0), true)),
        "midnight" => return Some((Time::midnight(), true)),
        _ => {}
    }
    let (clock, afternoon) = match ["am", "a.m.", "pm", "p.m."]
        .into_iter()
        .find_map(|suffix| Some((text.strip_suffix(suffix)?.trim_end(), suffix)))
    {
        Some((clock, suffix)) => (clock, Some(suffix.starts_with('p'))),
        None => (text, None),
    };
    let (hour, minute) = match clock.split_once(':') {
        Some((hour, minute)) if minute.len() == 2 => (number(hour)?, number(minute)?),
        Some(_) => return None,
        None => (number(clock)?, 0),
    };
    let hour = match afternoon {
        None => hour,
        Some(_) if !(1..=12).contains(&hour) => return None,
        Some(false) => hour % 12,
        Some(true) => hour % 12 + 12,
    };
    let clear = afternoon.is_some() || clock.contains(':');
    Some((Time::new(hour, minute, 0, 0).ok()?, clear))
}

/// One or two ASCII digits.
fn number(text: &str) -> Option<i8> {
    if (1..=2).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: i8, minute: i8) -> Time {
        Time::new(hour, minute, 0, 0).unwrap()
    }

    #[test]
    fn the_keyword_forms_give_now() {
        for (text, clear) in [
            ("time now in new york", true),
            ("time in new york", true),
            ("now in new york", false),
        ] {
            // Act
            let query = parse(text);

            // Assert
            let expected = Query {
                form: Form::Now,
                city: "new york".into(),
                clear,
            };
            assert_eq!(query, Some(expected), "{text}");
        }
    }

    #[test]
    fn a_time_before_in_is_the_time_in_the_city() {
        // Act
        let query = parse("17:00 in las vegas");

        // Assert
        let expected = Query {
            form: Form::InCity(at(17, 0)),
            city: "las vegas".into(),
            clear: true,
        };
        assert_eq!(query, Some(expected));
    }

    #[test]
    fn here_before_in_is_the_time_of_the_owner() {
        // Act
        let query = parse("6pm here in las vegas");

        // Assert
        let expected = Query {
            form: Form::Here(at(18, 0)),
            city: "las vegas".into(),
            clear: true,
        };
        assert_eq!(query, Some(expected));
    }

    #[test]
    fn the_first_in_ends_the_time_so_a_city_name_can_contain_in() {
        // Act
        let query = parse("17:00 in kirkby in ashfield").unwrap();

        // Assert
        assert_eq!(query.city, "kirkby in ashfield");
    }

    #[test]
    fn the_time_forms_give_the_time_and_the_clear_flag() {
        for (text, time, clear) in [
            ("18", at(18, 0), false),
            ("6", at(6, 0), false),
            ("0", at(0, 0), false),
            ("18:30", at(18, 30), true),
            ("0:05", at(0, 5), true),
            ("6am", at(6, 0), true),
            ("6 am", at(6, 0), true),
            ("6:30pm", at(18, 30), true),
            ("6 p.m.", at(18, 0), true),
            ("12am", at(0, 0), true),
            ("12:15 am", at(0, 15), true),
            ("12pm", at(12, 0), true),
            ("noon", at(12, 0), true),
            ("midnight", at(0, 0), true),
        ] {
            // Act
            let parsed = parse_time(text);

            // Assert
            assert_eq!(parsed, Some((time, clear)), "{text}");
        }
    }

    #[test]
    fn text_that_is_not_a_time_gives_none() {
        for text in [
            "", "24", "25:00", "6:7", "6:60", "123", "13pm", "0am", "pm", "6 pam", "five", "5 gib",
            "-1", "+6", "6:", ":30",
        ] {
            // Act
            let parsed = parse_time(text);

            // Assert
            assert_eq!(parsed, None, "{text:?}");
        }
    }

    #[test]
    fn text_with_no_city_after_in_gives_none() {
        for text in ["firefox in", "log in", "17:00 in", "in", "time now", "now"] {
            // Act
            let query = parse(text);

            // Assert
            assert_eq!(query, None, "{text:?}");
        }
    }
}
