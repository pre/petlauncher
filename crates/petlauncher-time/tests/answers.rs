use std::path::Path;

use jiff::tz::{Offset, TimeZone};
use jiff::ZonedDateTime;
use petlauncher_time::{Answer, Cities};

fn fixture_cities() -> Cities {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    Cities::load(&fixtures.join("zoneinfo"), &fixtures.join("Locations.xml")).unwrap()
}

fn at(time: &str) -> ZonedDateTime {
    time.parse().unwrap()
}

fn titles(answers: &[Answer]) -> Vec<&str> {
    answers.iter().map(|answer| answer.title.as_str()).collect()
}

fn subtitles(answers: &[Answer]) -> Vec<&str> {
    answers
        .iter()
        .map(|answer| answer.subtitle.as_str())
        .collect()
}

// R20, query 1: `time now in new york`.
#[test]
fn time_now_in_new_york_gives_the_time_in_new_york() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T18:04[Europe/Helsinki]");

    // Act
    let answers = cities.answer("time now in new york", &now);

    // Assert
    assert_eq!(
        answers,
        [Answer {
            title: "11:04, Thursday".into(),
            subtitle: "Now in New York, New York, United States (EST, UTC-5), \
                       7 h behind Helsinki"
                .into(),
            copy: Some("11:04, Thursday".into()),
        }]
    );
}

#[test]
fn time_in_and_now_in_give_the_same_answer_as_time_now_in() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T18:04[Europe/Helsinki]");
    let expected = cities.answer("time now in new york", &now);

    // Act
    let time_in = cities.answer("time in new york", &now);
    let now_in = cities.answer("now in new york", &now);

    // Assert
    assert_eq!(time_in, expected);
    assert_eq!(now_in, expected);
}

// R20, query 2: `17:00 in las vegas`.
#[test]
fn a_time_in_las_vegas_gives_the_owner_time_for_each_las_vegas() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T12:00[Europe/Helsinki]");

    // Act
    let answers = cities.answer("17:00 in las vegas", &now);

    // Assert
    assert_eq!(titles(&answers), ["03:00, Friday", "02:00, Friday"]);
    assert_eq!(
        subtitles(&answers),
        [
            "17:00 in Las Vegas, Nevada, United States (PST, UTC-8) \
             is 03:00 tomorrow in Helsinki",
            "17:00 in Las Vegas, New Mexico, United States (MST, UTC-7) \
             is 02:00 tomorrow in Helsinki",
        ]
    );
    assert_eq!(answers[0].copy.as_deref(), Some("03:00, Friday"));
}

// R20, query 3: `6am in helsinki`.
#[test]
fn six_am_in_helsinki_gives_the_owner_time() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T09:00[America/New_York]");

    // Act
    let answers = cities.answer("6am in helsinki", &now);

    // Assert
    assert_eq!(titles(&answers), ["23:00, Wednesday"]);
    assert_eq!(
        subtitles(&answers),
        ["06:00 in Helsinki, Finland (EET, UTC+2) is 23:00 yesterday in New York"]
    );
}

#[test]
fn the_forms_of_six_in_the_morning_give_the_same_answer() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T09:00[America/New_York]");
    let expected = cities.answer("6am in helsinki", &now);

    for query in [
        "6 am in helsinki",
        "6:00am in helsinki",
        "6:00 a.m. in helsinki",
        "06:00 in helsinki",
        "6 in helsinki",
        "6AM IN HELSINKI",
    ] {
        // Act
        let answers = cities.answer(query, &now);

        // Assert
        assert_eq!(answers, expected, "{query}");
    }
}

#[test]
fn pm_noon_and_midnight_give_the_correct_time_of_day() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T09:00[Europe/Helsinki]");

    for (query, title) in [
        ("6:30pm in helsinki", "18:30, Thursday"),
        ("6:30 p.m. in helsinki", "18:30, Thursday"),
        ("12am in helsinki", "00:00, Thursday"),
        ("12pm in helsinki", "12:00, Thursday"),
        ("noon in helsinki", "12:00, Thursday"),
        ("midnight in helsinki", "00:00, Thursday"),
        ("18:30 in helsinki", "18:30, Thursday"),
        ("18 in helsinki", "18:00, Thursday"),
    ] {
        // Act
        let answers = cities.answer(query, &now);

        // Assert
        assert_eq!(titles(&answers), [title], "{query}");
    }
}

#[test]
fn daylight_saving_time_in_one_zone_changes_the_answer() {
    // Arrange: New York starts daylight saving time on 8 March 2026,
    // Helsinki on 29 March 2026.
    let cities = fixture_cities();
    let before = at("2026-03-05T12:00[Europe/Helsinki]");
    let after = at("2026-03-10T12:00[Europe/Helsinki]");

    // Act
    let answers_before = cities.answer("17:00 in new york", &before);
    let answers_after = cities.answer("17:00 in new york", &after);

    // Assert
    assert_eq!(
        subtitles(&answers_before),
        ["17:00 in New York, New York, United States (EST, UTC-5) is 00:00 tomorrow in Helsinki"]
    );
    assert_eq!(
        subtitles(&answers_after),
        ["17:00 in New York, New York, United States (EDT, UTC-4) is 23:00 today in Helsinki"]
    );
}

#[test]
fn the_date_line_gives_the_same_clock_time_on_the_next_day() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T10:00[Pacific/Honolulu]");

    // Act
    let answers = cities.answer("now in kiritimati", &now);

    // Assert
    assert_eq!(titles(&answers), ["10:00, Friday"]);
    assert_eq!(
        subtitles(&answers),
        ["Now in Kiritimati, Kiribati (UTC+14), 24 h ahead of Honolulu"]
    );
}

#[test]
fn a_time_across_the_date_line_is_on_the_day_of_that_city() {
    // Arrange: when it is Thursday 10:00 in Honolulu, it is Friday in
    // Kiritimati.
    let cities = fixture_cities();
    let now = at("2026-11-26T10:00[Pacific/Honolulu]");

    // Act
    let answers = cities.answer("09:00 in kiritimati", &now);

    // Assert
    assert_eq!(titles(&answers), ["09:00, Thursday"]);
    assert_eq!(
        subtitles(&answers),
        ["09:00 in Kiritimati, Kiribati (UTC+14) is 09:00 today in Honolulu"]
    );
}

#[test]
fn a_time_that_does_not_exist_gives_the_time_after_the_change() {
    // Arrange: on 8 March 2026 the clocks in New York go from 02:00 to 03:00.
    let cities = fixture_cities();
    let now = at("2026-03-08T12:00[Europe/Helsinki]");

    // Act
    let answers = cities.answer("02:30 in new york", &now);

    // Assert
    assert_eq!(titles(&answers), ["09:30, Sunday"]);
    assert_eq!(
        subtitles(&answers),
        [
            "02:30 does not exist today in New York, New York, United States, \
             the clocks go forward. 03:30 (EDT, UTC-4) is 09:30 today in Helsinki"
        ]
    );
}

#[test]
fn a_time_that_exists_two_times_gives_two_answers() {
    // Arrange: on 1 November 2026 the clocks in New York go from 02:00 back
    // to 01:00.
    let cities = fixture_cities();
    let now = at("2026-11-01T12:00[Europe/Helsinki]");

    // Act
    let answers = cities.answer("01:30 in new york", &now);

    // Assert
    assert_eq!(titles(&answers), ["07:30, Sunday", "08:30, Sunday"]);
    assert_eq!(
        subtitles(&answers),
        [
            "01:30 in New York, New York, United States (EDT, UTC-4, first of two) \
             is 07:30 today in Helsinki",
            "01:30 in New York, New York, United States (EST, UTC-5, second of two) \
             is 08:30 today in Helsinki",
        ]
    );
}

#[test]
fn the_here_form_gives_the_city_time_for_an_owner_time() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T12:00[Europe/Helsinki]");

    // Act
    let answers = cities.answer("17:00 here in las vegas", &now);

    // Assert
    assert_eq!(titles(&answers), ["07:00, Thursday", "08:00, Thursday"]);
    assert_eq!(
        subtitles(&answers)[0],
        "17:00 in Helsinki is 07:00 today in Las Vegas, Nevada, United States (PST, UTC-8)"
    );
}

#[test]
fn the_here_form_shows_the_owner_zone_when_the_owner_time_exists_two_times() {
    // Arrange: on 25 October 2026 the clocks in Helsinki go from 04:00 back
    // to 03:00.
    let cities = fixture_cities();
    let now = at("2026-10-25T12:00[Europe/Helsinki]");

    // Act
    let answers = cities.answer("03:30 here in new york", &now);

    // Assert
    assert_eq!(
        subtitles(&answers),
        [
            "03:30 in Helsinki (EEST, UTC+3, first of two) is 20:30 yesterday \
             in New York, New York, United States (EDT, UTC-4)",
            "03:30 in Helsinki (EET, UTC+2, second of two) is 21:30 yesterday \
             in New York, New York, United States (EDT, UTC-4)",
        ]
    );
}

#[test]
fn a_city_with_a_half_hour_offset_shows_the_minutes() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T18:04[Europe/Helsinki]");

    // Act
    let answers = cities.answer("now in kolkata", &now);

    // Assert
    assert_eq!(titles(&answers), ["21:34, Thursday"]);
    assert_eq!(
        subtitles(&answers),
        ["Now in Kolkata, West Bengal, India (IST, UTC+5:30), 3 h 30 min ahead of Helsinki"]
    );
}

#[test]
fn a_city_in_the_owner_zone_has_the_same_time() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T18:04[Europe/Helsinki]");

    // Act
    let answers = cities.answer("now in helsinki", &now);

    // Assert
    assert_eq!(
        subtitles(&answers),
        ["Now in Helsinki, Finland (EET, UTC+2), same time as Helsinki"]
    );
}

#[test]
fn an_owner_zone_with_no_name_is_your_time_zone() {
    // Arrange
    let cities = fixture_cities();
    let owner = TimeZone::fixed(Offset::from_hours(2).unwrap());
    let now = at("2026-11-26T12:00[Europe/Helsinki]").with_time_zone(owner);

    // Act
    let answers = cities.answer("17:00 in las vegas", &now);

    // Assert
    assert_eq!(
        subtitles(&answers)[0],
        "17:00 in Las Vegas, Nevada, United States (PST, UTC-8) \
         is 03:00 tomorrow in your time zone"
    );
}

#[test]
fn one_row_is_given_for_each_zone_of_the_cities_with_the_name() {
    // Arrange: Paris, Texas has the same zone as Paris, Illinois.
    let cities = fixture_cities();
    let now = at("2026-11-26T12:00[Europe/Helsinki]");

    // Act
    let answers = cities.answer("17:00 in paris", &now);

    // Assert
    assert_eq!(
        subtitles(&answers),
        [
            "17:00 in Paris, France (CET, UTC+1) is 18:00 today in Helsinki",
            "17:00 in Paris, Illinois, United States (CST, UTC-6) is 01:00 tomorrow in Helsinki",
        ]
    );
}

#[test]
fn text_that_is_not_a_time_query_gives_no_answer() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T12:00[Europe/Helsinki]");

    for text in [
        "",
        "firefox",
        "firefox in",
        "5 GiB in MB",
        "log in",
        "10 in 20",
        "in helsinki",
        "17:00 in",
        "time",
        "time now",
        "now in stock",
        "18 in atlantis",
        "25:00 in helsinki",
        "24 in helsinki",
        "13pm in helsinki",
        "0am in helsinki",
        "6:7 in helsinki",
        "6:60 in helsinki",
        "six in helsinki",
    ] {
        // Act
        let answers = cities.answer(text, &now);

        // Assert
        assert!(answers.is_empty(), "{text:?} gives {answers:?}");
    }
}

#[test]
fn a_clear_time_query_with_an_unknown_city_gives_a_not_found_row() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T12:00[Europe/Helsinki]");

    for text in [
        "17:00 in atlantis",
        "6am in atlantis",
        "noon in atlantis",
        "time in atlantis",
        "time now in atlantis",
        "17:00 here in atlantis",
    ] {
        // Act
        let answers = cities.answer(text, &now);

        // Assert
        assert_eq!(
            answers,
            [Answer {
                title: "City not found: atlantis".into(),
                subtitle: "petlauncher has no time zone for this city name".into(),
                copy: None,
            }],
            "{text}"
        );
    }
}

#[test]
fn spaces_and_capital_letters_do_not_change_the_answer() {
    // Arrange
    let cities = fixture_cities();
    let now = at("2026-11-26T18:04[Europe/Helsinki]");
    let expected = cities.answer("time now in new york", &now);

    // Act
    let answers = cities.answer("  Time  Now In  NEW   York ", &now);

    // Assert
    assert_eq!(answers, expected);
}
