use std::path::{Path, PathBuf};

use petlauncher_time::Cities;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fixture_cities() -> Cities {
    Cities::load(
        &fixtures().join("zoneinfo"),
        &fixtures().join("Locations.xml"),
    )
    .unwrap()
}

/// Each match as "name, place, zone", in the match order.
fn found(cities: &Cities, name: &str) -> Vec<String> {
    cities
        .find(name)
        .iter()
        .map(|city| {
            let zone = city.time_zone.iana_name().unwrap_or("?");
            format!("{}, {}, {}", city.name, city.place, zone)
        })
        .collect()
}

#[test]
fn las_vegas_gives_nevada_first_and_new_mexico_second() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let las_vegas = found(&cities, "las vegas");

    // Assert
    assert_eq!(
        las_vegas,
        [
            "Las Vegas, Nevada, United States, America/Los_Angeles",
            "Las Vegas, New Mexico, United States, America/Denver",
        ]
    );
}

#[test]
fn the_tzdb_city_comes_first_and_the_gnome_cities_follow_in_file_order() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let paris = found(&cities, "paris");

    // Assert
    assert_eq!(
        paris,
        [
            "Paris, France, Europe/Paris",
            "Paris, Illinois, United States, America/Chicago",
            "Paris, Texas, United States, America/Chicago",
        ]
    );
}

#[test]
fn the_tzdb_zone_replaces_an_old_gnome_hint_of_the_same_city() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let chihuahua = found(&cities, "chihuahua");

    // Assert
    assert_eq!(
        chihuahua,
        ["Chihuahua, Chihuahua, Mexico, America/Chihuahua"]
    );
}

#[test]
fn a_far_city_with_the_name_of_a_tzdb_city_in_the_same_country_stays_apart() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let juneau = found(&cities, "juneau");

    // Assert
    assert_eq!(
        juneau,
        [
            "Juneau, Alaska, United States, America/Juneau",
            "Juneau, Wisconsin, United States, America/Chicago",
        ]
    );
}

#[test]
fn a_tzdb_city_gets_the_names_of_the_gnome_city() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let new_york = found(&cities, "new york");
    let kolkata = found(&cities, "kolkata");

    // Assert
    assert_eq!(
        new_york,
        ["New York, New York, United States, America/New_York"]
    );
    assert_eq!(kolkata, ["Kolkata, West Bengal, India, Asia/Kolkata"]);
}

#[test]
fn saint_and_st_are_the_same_word_so_the_two_sources_merge() {
    // Arrange: the tzdb zone is America/St_Johns, and the GNOME city is
    // "Saint John's".
    let cities = fixture_cities();

    // Act
    let short = found(&cities, "st. john's");
    let long = found(&cities, "saint johns");

    // Assert
    let expected = ["Saint John's, Newfoundland and Labrador, Canada, America/St_Johns"];
    assert_eq!(short, expected);
    assert_eq!(long, expected);
}

#[test]
fn a_city_only_in_the_tzdb_gets_the_country_name_of_iso3166_tab() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let kiritimati = found(&cities, "kiritimati");
    let dubai = found(&cities, "dubai");

    // Assert
    assert_eq!(kiritimati, ["Kiritimati, Kiribati, Pacific/Kiritimati"]);
    assert_eq!(dubai, ["Dubai, United Arab Emirates, Asia/Dubai"]);
}

#[test]
fn the_nearest_enclosing_tz_hint_gives_the_zone() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let from_city = found(&cities, "aqtobe");
    let from_country = found(&cities, "kigali");

    // Assert
    assert_eq!(from_city, ["Aqtöbe, Kazakhstan, Asia/Aqtobe"]);
    assert_eq!(from_country, ["Kigali, Rwanda, Africa/Kigali"]);
}

#[test]
fn the_match_ignores_case_accents_and_punctuation() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let names = ["GODTHAB", "godthåb", "Las-Vegas", "  las   vegas "];

    // Assert
    for name in names {
        assert!(!cities.find(name).is_empty(), "{name} is not found");
    }
}

#[test]
fn stations_time_zone_names_and_named_zones_are_not_cities() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let names = [
        "helsinki-vantaa",
        "mccarran international airport",
        "eastern kazakhstan",
        "coordinated universal time (utc)",
        "finland",
    ];

    // Assert
    for name in names {
        assert!(cities.find(name).is_empty(), "{name} is found");
    }
}

#[test]
fn a_city_whose_zone_is_not_in_the_database_is_dropped() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let olympus = found(&cities, "olympus");

    // Assert
    assert!(olympus.is_empty());
}

#[test]
fn a_missing_gnome_file_gives_the_tzdb_cities_only() {
    // Arrange
    let missing = fixtures().join("no-such-file.xml");

    // Act
    let cities = Cities::load(&fixtures().join("zoneinfo"), &missing).unwrap();

    // Assert
    assert_eq!(
        found(&cities, "new york"),
        ["New York, United States, America/New_York"]
    );
    assert!(cities.find("las vegas").is_empty());
}

#[test]
fn a_malformed_gnome_file_is_an_error() {
    // Arrange
    let broken = fixtures().join("broken-locations.xml");

    // Act
    let result = Cities::load(&fixtures().join("zoneinfo"), &broken);

    // Assert
    let message = result.err().unwrap().to_string();
    assert!(message.contains("broken-locations.xml"), "{message}");
}

#[test]
fn a_gnome_path_that_cannot_be_read_is_an_error() {
    // Arrange: a directory exists but is not a file.
    let directory = fixtures().join("zoneinfo");

    // Act
    let result = Cities::load(&fixtures().join("zoneinfo"), &directory);

    // Assert
    let message = result.err().unwrap().to_string();
    assert!(message.starts_with("cannot read "), "{message}");
}

#[test]
fn a_missing_zone_tab_is_an_error() {
    // Arrange
    let no_zoneinfo = fixtures().join("no-such-directory");

    // Act
    let result = Cities::load(&no_zoneinfo, &fixtures().join("Locations.xml"));

    // Assert
    let message = result.err().unwrap().to_string();
    assert!(message.contains("zone.tab"), "{message}");
}

#[test]
fn the_table_counts_each_loaded_city() {
    // Arrange
    let cities = fixture_cities();

    // Act
    let count = cities.len();

    // Assert: 15 zones of zone.tab, and 7 cities of the GNOME file that are
    // not merged into a zone: Aqtöbe, Godthåb, Paris (Illinois and Texas),
    // Las Vegas (Nevada and New Mexico) and Juneau (Wisconsin). Olympus is
    // dropped.
    assert_eq!(count, 22);
    assert!(!cities.is_empty());
}

#[test]
fn the_table_can_be_loaded_in_a_different_thread() {
    // Arrange
    fn send_and_sync<T: Send + Sync>() {}

    // Act and assert: the test compiles only if the type is Send and Sync.
    send_and_sync::<Cities>();
}

/// This test reads the real system files. It does nothing on a computer
/// that does not have the GNOME city list.
#[test]
fn the_system_files_give_las_vegas_and_all_cities_have_a_zone() {
    // Arrange
    if !Path::new(petlauncher_time::GWEATHER_LOCATIONS).exists() {
        return;
    }

    // Act
    let cities = Cities::load_system().unwrap();

    // Assert
    assert!(cities.len() > 4000, "{} cities", cities.len());
    assert_eq!(
        found(&cities, "las vegas")[0],
        "Las Vegas, Nevada, United States, America/Los_Angeles"
    );
    assert_eq!(
        found(&cities, "helsinki"),
        ["Helsinki, Finland, Europe/Helsinki"]
    );
}
