//! The cities of the time zone database. `zone.tab` gives a zone, its
//! country and its coordinates on each line. The city is the last part of the
//! zone name. `iso3166.tab` gives the country names.

use std::collections::HashMap;

use crate::cities::Place;

pub(crate) fn places(zone_tab: &str, iso3166_tab: &str) -> Vec<Place> {
    let countries: HashMap<&str, &str> = rows(iso3166_tab)
        .filter_map(|fields| Some((*fields.first()?, *fields.get(1)?)))
        .collect();
    rows(zone_tab)
        .filter_map(|fields| {
            let [code, coordinates, zone, ..] = fields[..] else {
                return None;
            };
            Some(Place {
                name: zone.rsplit('/').next()?.replace('_', " "),
                state: None,
                country_code: code.to_owned(),
                country: countries.get(code).unwrap_or(&code).to_string(),
                time_zone: zone.to_owned(),
                position: iso6709(coordinates),
            })
        })
        .collect()
}

/// The tab-separated fields of each line that is not empty or a comment.
fn rows(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .map(|line| line.split('\t').collect())
}

/// Reads the ISO 6709 coordinates of zone.tab, for example `+6010+02458` or
/// `+404251-0740023`, as latitude and longitude in degrees.
fn iso6709(text: &str) -> Option<(f64, f64)> {
    let longitude_start = text.get(1..)?.find(['+', '-'])? + 1;
    let (latitude, longitude) = text.split_at(longitude_start);
    Some((degrees(latitude, 2)?, degrees(longitude, 3)?))
}

/// Reads a sign, `degree_digits` digits of degrees, two digits of minutes and
/// two optional digits of seconds.
fn degrees(text: &str, degree_digits: usize) -> Option<f64> {
    let sign = if text.starts_with('-') { -1.0 } else { 1.0 };
    let digits = text.get(1..)?;
    let part = |start: usize, end: usize| digits.get(start..end)?.parse::<f64>().ok();
    let minutes_end = degree_digits + 2;
    let seconds = match digits.len() {
        length if length == minutes_end => 0.0,
        length if length == minutes_end + 2 => part(minutes_end, length)?,
        _ => return None,
    };
    let value =
        part(0, degree_digits)? + part(degree_digits, minutes_end)? / 60.0 + seconds / 3600.0;
    Some(sign * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zone_line_gives_the_city_country_and_position() {
        // Arrange
        let zone_tab = "# comment\n\nUS\t+404251-0740023\tAmerica/New_York\tEastern (most areas)\n";
        let iso3166_tab = "# comment\nUS\tUnited States\n";

        // Act
        let places = places(zone_tab, iso3166_tab);

        // Assert
        let [place] = &places[..] else {
            panic!("{} places", places.len());
        };
        assert_eq!(place.name, "New York");
        assert_eq!(place.country, "United States");
        assert_eq!(place.country_code, "US");
        assert_eq!(place.time_zone, "America/New_York");
        let (latitude, longitude) = place.position.unwrap();
        assert!((latitude - 40.714).abs() < 0.001, "{latitude}");
        assert!((longitude + 74.006).abs() < 0.001, "{longitude}");
    }

    #[test]
    fn a_zone_with_an_unknown_country_code_uses_the_code_as_the_country() {
        // Act
        let places = places("XX\t+6010+02458\tEurope/Nowhere\n", "");

        // Assert
        assert_eq!(places[0].country, "XX");
    }

    #[test]
    fn a_line_with_too_few_fields_is_ignored() {
        // Act
        let places = places("FI\t+6010+02458\n", "");

        // Assert
        assert!(places.is_empty());
    }

    #[test]
    fn coordinates_in_degrees_and_minutes_are_read() {
        // Act
        let position = iso6709("-3352+15113");

        // Assert
        let (latitude, longitude) = position.unwrap();
        assert!((latitude + 33.8667).abs() < 0.001, "{latitude}");
        assert!((longitude - 151.2167).abs() < 0.001, "{longitude}");
    }

    #[test]
    fn malformed_coordinates_give_none() {
        for text in [
            "",
            "+",
            "+6010",
            "+60+024",
            "+6010+0245",
            "+60x0+02458",
            "+6010+0245812345",
        ] {
            // Act
            let position = iso6709(text);

            // Assert
            assert_eq!(position, None, "{text:?}");
        }
    }
}
