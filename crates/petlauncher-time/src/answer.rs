use std::cmp::Ordering;

use jiff::civil::Time;
use jiff::tz::{AmbiguousOffset, TimeZone};
use jiff::ZonedDateTime;

use crate::cities::{Cities, City};
use crate::query::{self, Form};

/// One row for the user interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The main text, for example `03:00, Friday`.
    pub title: String,
    /// The second text, for example `17:00 in Las Vegas, Nevada, United
    /// States (PST, UTC-8) is 03:00 tomorrow in Helsinki`.
    pub subtitle: String,
    /// The text that Enter copies, or `None` when Enter does nothing.
    pub copy: Option<String>,
}

impl Cities {
    /// Gives the rows for a query, or no rows when the text is not a time
    /// query. `now` is the current time in the owner's time zone.
    pub fn answer(&self, text: &str, now: &ZonedDateTime) -> Vec<Answer> {
        let Some(query) = query::parse(text) else {
            return Vec::new();
        };
        let cities = one_city_for_each_zone(self.find(&query.city));
        if cities.is_empty() {
            return if query.clear {
                vec![not_found(&query.city)]
            } else {
                Vec::new()
            };
        }
        let owner = Side::owner(now.time_zone());
        cities
            .into_iter()
            .flat_map(|city| {
                let city = Side::city(city);
                match query.form {
                    Form::Now => vec![time_now(&city, &owner, now)],
                    Form::InCity(time) => convert(time, &city, &owner, now),
                    Form::Here(time) => convert(time, &owner, &city, now),
                }
            })
            .collect()
    }
}

/// Two cities with the same name and zone give the same row, so only the
/// first one is kept.
fn one_city_for_each_zone(cities: Vec<&City>) -> Vec<&City> {
    let mut kept: Vec<&City> = Vec::new();
    for city in cities {
        if kept
            .iter()
            .all(|other| other.time_zone.iana_name() != city.time_zone.iana_name())
        {
            kept.push(city);
        }
    }
    kept
}

/// The owner's zone or a city, in a sentence.
struct Side {
    time_zone: TimeZone,
    /// "Las Vegas, Nevada, United States", or "Helsinki" for the owner.
    label: String,
    /// A city shows its zone abbreviation and offset. The owner knows them.
    shows_zone: bool,
}

impl Side {
    fn city(city: &City) -> Side {
        let label = match city.place.as_str() {
            "" => city.name.clone(),
            place => format!("{}, {place}", city.name),
        };
        Side {
            time_zone: city.time_zone.clone(),
            label,
            shows_zone: true,
        }
    }

    /// The label is the city part of the zone name, for example "Helsinki"
    /// for `Europe/Helsinki`.
    fn owner(time_zone: &TimeZone) -> Side {
        let label = match time_zone.iana_name() {
            Some(name) => name.rsplit('/').next().unwrap_or(name).replace('_', " "),
            None => "your time zone".to_owned(),
        };
        Side {
            time_zone: time_zone.clone(),
            label,
            shows_zone: false,
        }
    }

    /// For example " (PST, UTC-8)", or "" for the owner. `extra` adds a last
    /// part and also shows the owner's zone.
    fn zone_note(&self, zoned: &ZonedDateTime, extra: Option<&str>) -> String {
        if !self.shows_zone && extra.is_none() {
            return String::new();
        }
        let info = self.time_zone.to_offset_info(zoned.timestamp());
        // A zone with no abbreviation gives the offset, for example "+14".
        let abbreviation =
            Some(info.abbreviation()).filter(|text| text.starts_with(char::is_alphabetic));
        let offset = utc_offset(zoned);
        let parts: Vec<&str> = abbreviation
            .into_iter()
            .chain([offset.as_str()])
            .chain(extra)
            .collect();
        format!(" ({})", parts.join(", "))
    }
}

fn time_now(city: &Side, owner: &Side, now: &ZonedDateTime) -> Answer {
    let there = now.with_time_zone(city.time_zone.clone());
    let difference = there.offset().seconds() - now.offset().seconds();
    let relation = match difference.cmp(&0) {
        Ordering::Equal => format!("same time as {}", owner.label),
        Ordering::Greater => format!("{} ahead of {}", duration(difference), owner.label),
        Ordering::Less => format!("{} behind {}", duration(-difference), owner.label),
    };
    let title = clock_and_weekday(&there);
    Answer {
        subtitle: format!(
            "Now in {}{}, {relation}",
            city.label,
            city.zone_note(&there, None)
        ),
        copy: Some(title.clone()),
        title,
    }
}

/// Converts `time` on today's date in the zone of `from` to the zone of `to`.
fn convert(time: Time, from: &Side, to: &Side, now: &ZonedDateTime) -> Vec<Answer> {
    let date = now.with_time_zone(from.time_zone.clone()).date();
    let local = from
        .time_zone
        .to_ambiguous_zoned_datetime(date.to_datetime(time));
    let clock = time.strftime("%H:%M");
    let start_text = |start: &ZonedDateTime, extra: Option<&str>| {
        format!("{clock} in {}{}", from.label, from.zone_note(start, extra))
    };
    let starts = match local.offset() {
        AmbiguousOffset::Unambiguous { .. } => local
            .compatible()
            .map(|start| vec![(start_text(&start, None), start)]),
        // The compatible time is after the change, for example 03:30 for 02:30.
        AmbiguousOffset::Gap { .. } => local.compatible().map(|start| {
            let text = format!(
                "{clock} does not exist today in {}, the clocks go forward. {}{}",
                from.label,
                start.strftime("%H:%M"),
                from.zone_note(&start, None)
            );
            vec![(text, start)]
        }),
        AmbiguousOffset::Fold { .. } => local.clone().earlier().and_then(|first| {
            let second = local.later()?;
            Ok(vec![
                (start_text(&first, Some("first of two")), first),
                (start_text(&second, Some("second of two")), second),
            ])
        }),
    };
    // The conversion fails only at the limits of the supported dates.
    starts
        .unwrap_or_default()
        .iter()
        .map(|(text, start)| converted(text, start, to, now))
        .collect()
}

/// The row for `start` in the zone of `to`. `start_text` is the first part of
/// the second text.
fn converted(start_text: &str, start: &ZonedDateTime, to: &Side, now: &ZonedDateTime) -> Answer {
    let end = start.with_time_zone(to.time_zone.clone());
    let today = now.with_time_zone(to.time_zone.clone()).date();
    let day = match (end.date() - today).get_days() {
        0 => "today".to_owned(),
        1 => "tomorrow".to_owned(),
        -1 => "yesterday".to_owned(),
        _ => end.strftime("on %A").to_string(),
    };
    let title = clock_and_weekday(&end);
    Answer {
        subtitle: format!(
            "{start_text} is {} {day} in {}{}",
            end.strftime("%H:%M"),
            to.label,
            to.zone_note(&end, None)
        ),
        copy: Some(title.clone()),
        title,
    }
}

fn not_found(city: &str) -> Answer {
    Answer {
        title: format!("City not found: {city}"),
        subtitle: "petlauncher has no time zone for this city name".to_owned(),
        copy: None,
    }
}

fn clock_and_weekday(zoned: &ZonedDateTime) -> String {
    zoned.strftime("%H:%M, %A").to_string()
}

/// For example "UTC-8", "UTC+5:30" or "UTC+0".
fn utc_offset(zoned: &ZonedDateTime) -> String {
    let seconds = zoned.offset().seconds();
    let sign = if seconds < 0 { '-' } else { '+' };
    let (hours, minutes) = (seconds.abs() / 3600, seconds.abs() % 3600 / 60);
    match minutes {
        0 => format!("UTC{sign}{hours}"),
        _ => format!("UTC{sign}{hours}:{minutes:02}"),
    }
}

/// For example "7 h", "3 h 30 min" or "45 min".
fn duration(seconds: i32) -> String {
    let (hours, minutes) = (seconds / 3600, seconds % 3600 / 60);
    match (hours, minutes) {
        (0, _) => format!("{minutes} min"),
        (_, 0) => format!("{hours} h"),
        _ => format!("{hours} h {minutes} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_duration_shows_hours_and_minutes() {
        for (seconds, text) in [(25_200, "7 h"), (12_600, "3 h 30 min"), (2_700, "45 min")] {
            // Act
            let duration = duration(seconds);

            // Assert
            assert_eq!(duration, text);
        }
    }

    #[test]
    fn an_offset_shows_minutes_only_when_they_are_not_zero() {
        for (zoned, text) in [
            ("2026-11-26T12:00[America/Los_Angeles]", "UTC-8"),
            ("2026-11-26T12:00[Asia/Kolkata]", "UTC+5:30"),
            ("2026-11-26T12:00[America/St_Johns]", "UTC-3:30"),
            ("2026-11-26T12:00[UTC]", "UTC+0"),
        ] {
            // Arrange
            let zoned: ZonedDateTime = zoned.parse().unwrap();

            // Act
            let offset = utc_offset(&zoned);

            // Assert
            assert_eq!(offset, text);
        }
    }
}
