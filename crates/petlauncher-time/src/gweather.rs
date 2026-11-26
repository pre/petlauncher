//! The cities of `Locations.xml` of libgweather 4.
//!
//! A `<city>` is in a `<country>`, and some are also in a `<state>`. The zone
//! of a city is the `<tz-hint>` of the city or, when the city has none, of
//! the nearest enclosing element that has one. The other elements, for
//! example the weather stations in `<location>`, are not cities.

use roxmltree::{Document, Node, ParsingOptions};

use crate::cities::Place;

pub(crate) fn places(xml: &str) -> Result<Vec<Place>, roxmltree::Error> {
    let options = ParsingOptions {
        allow_dtd: true,
        ..ParsingOptions::default()
    };
    let document = Document::parse_with_options(xml, options)?;
    Ok(document
        .descendants()
        .filter(|node| node.has_tag_name("city"))
        .filter_map(place)
        .collect())
}

fn place(city: Node) -> Option<Place> {
    let enclosing = |tag: &str| city.ancestors().find(|node| node.has_tag_name(tag));
    let country = enclosing("country");
    Some(Place {
        name: name(city)?,
        state: enclosing("state").and_then(name),
        country_code: country
            .and_then(|country| child_text(country, "iso-code"))
            .unwrap_or_default()
            .to_owned(),
        country: country.and_then(name).unwrap_or_default(),
        time_zone: city
            .ancestors()
            .find_map(|node| child_text(node, "tz-hint"))?
            .to_owned(),
        position: child_text(city, "coordinates").and_then(latitude_longitude),
    })
}

/// The `<_name>`, which is translatable, or the `<name>` of an element.
fn name(node: Node) -> Option<String> {
    child_text(node, "_name")
        .or_else(|| child_text(node, "name"))
        .map(str::to_owned)
}

fn child_text<'a>(node: Node<'a, '_>, tag: &str) -> Option<&'a str> {
    let text = node
        .children()
        .find(|child| child.has_tag_name(tag))?
        .text()?;
    Some(text.trim())
}

/// Reads `<coordinates>`, which has the latitude and longitude in degrees.
fn latitude_longitude(text: &str) -> Option<(f64, f64)> {
    let (latitude, longitude) = text.split_once(' ')?;
    Some((latitude.parse().ok()?, longitude.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_city_outside_a_country_has_no_country() {
        // Arrange
        let xml = "<gweather><city><name>Free</name><tz-hint>UTC</tz-hint></city></gweather>";

        // Act
        let places = places(xml).unwrap();

        // Assert
        assert_eq!(places[0].name, "Free");
        assert_eq!(places[0].country, "");
        assert_eq!(places[0].country_code, "");
        assert_eq!(places[0].position, None);
    }

    #[test]
    fn a_city_with_no_name_or_no_zone_is_ignored() {
        // Arrange
        let xml = "<gweather><country><_name>A</_name>\
                   <city><coordinates>1 2</coordinates><tz-hint>UTC</tz-hint></city>\
                   <city><_name>No zone</_name></city></country></gweather>";

        // Act
        let places = places(xml).unwrap();

        // Assert
        assert!(places.is_empty());
    }

    #[test]
    fn malformed_coordinates_give_no_position() {
        for text in ["", "60.1", "x 24.9", "60.1 y"] {
            // Act
            let position = latitude_longitude(text);

            // Assert
            assert_eq!(position, None, "{text:?}");
        }
    }
}
