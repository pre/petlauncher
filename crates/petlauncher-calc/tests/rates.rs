use std::fs;

use petlauncher_calc::{
    calculate, load_cache, parse_ecb, save_cache, Rates, RatesError, CACHE_FILE,
};

const FIXTURE: &str = include_str!("fixtures/eurofxref-daily.xml");

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

#[test]
fn ecb_file_gives_the_date_and_the_rates() {
    // Act
    let rates = parse_ecb(FIXTURE).unwrap();

    // Assert
    assert_eq!(rates.date(), "2026-11-25");
    assert_eq!(rates.currencies().count(), 14);
    assert!(rates.currencies().any(|code| code == "USD"));
}

#[test]
fn parsed_ecb_rates_convert_a_currency() {
    // Arrange
    let rates = parse_ecb(FIXTURE).unwrap();

    // Act
    let result = calculate("100 usd in eur", Some(&rates)).unwrap();

    // Assert
    assert_eq!(result.text(), "95.13 EUR");
    assert_eq!(
        result.note.as_deref(),
        Some("ECB reference rates of 2026-11-25")
    );
}

#[test]
fn text_that_is_not_xml_is_a_format_error() {
    let result = parse_ecb("<html>Service unavailable");

    assert!(matches!(result, Err(RatesError::Format(_))));
}

#[test]
fn ecb_file_without_a_date_is_a_format_error() {
    let xml = "<Envelope><Cube><Cube><Cube currency='USD' rate='1.05'/></Cube></Cube></Envelope>";

    let result = parse_ecb(xml);

    assert!(matches!(result, Err(RatesError::Format(_))));
}

#[test]
fn ecb_file_without_rates_is_a_format_error() {
    let xml = "<Envelope><Cube><Cube time='2026-11-25'></Cube></Cube></Envelope>";

    let result = parse_ecb(xml);

    assert!(matches!(result, Err(RatesError::Format(_))));
}

#[test]
fn ecb_file_with_a_bad_rate_is_a_format_error() {
    let xml = "<Envelope><Cube><Cube time='2026-11-25'><Cube currency='USD' rate='abc'/></Cube></Cube></Envelope>";

    let result = parse_ecb(xml);

    assert!(matches!(result, Err(RatesError::Format(_))));
}

#[test]
fn rates_reject_a_date_that_is_not_iso() {
    let result = Rates::new("25.11.2026", &[("USD", "1.05")]);

    assert!(matches!(result, Err(RatesError::Format(_))));
}

#[test]
fn rates_reject_a_rate_that_is_not_positive() {
    let result = Rates::new("2026-11-25", &[("USD", "0")]);

    assert!(matches!(result, Err(RatesError::Format(_))));
}

#[test]
fn missing_cache_file_gives_no_rates() {
    // Arrange
    let dir = temp_dir();

    // Act
    let result = load_cache(dir.path()).unwrap();

    // Assert
    assert_eq!(result, None);
}

#[test]
fn saved_cache_loads_the_same_rates() {
    // Arrange
    let dir = temp_dir();

    // Act
    let saved = save_cache(dir.path(), FIXTURE).unwrap();
    let loaded = load_cache(dir.path()).unwrap();

    // Assert
    assert_eq!(loaded, Some(saved));
    assert!(dir.path().join(CACHE_FILE).is_file());
}

#[test]
fn save_cache_makes_the_missing_directory() {
    // Arrange
    let dir = temp_dir();
    let cache = dir.path().join("cache").join("petlauncher");

    // Act
    save_cache(&cache, FIXTURE).unwrap();

    // Assert
    assert!(cache.join(CACHE_FILE).is_file());
}

#[test]
fn bad_download_does_not_replace_the_cache() {
    // Arrange
    let dir = temp_dir();
    save_cache(dir.path(), FIXTURE).unwrap();

    // Act
    let result = save_cache(dir.path(), "<html>Gateway timeout</html>");

    // Assert
    assert!(matches!(result, Err(RatesError::Format(_))));
    assert_eq!(
        fs::read_to_string(dir.path().join(CACHE_FILE)).unwrap(),
        FIXTURE
    );
}

#[test]
fn damaged_cache_file_is_a_format_error() {
    // Arrange
    let dir = temp_dir();
    fs::write(dir.path().join(CACHE_FILE), "not xml").unwrap();

    // Act
    let result = load_cache(dir.path());

    // Assert
    assert!(matches!(result, Err(RatesError::Format(_))));
}
