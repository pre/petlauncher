use petlauncher_calc::{calculate, is_calculation, CalcError, Calculation, Dimension, Rates};

fn calc(query: &str) -> Calculation {
    calculate(query, None).unwrap_or_else(|| panic!("`{query}` is not a calculation"))
}

fn text(query: &str) -> String {
    calc(query).text()
}

fn error(query: &str) -> CalcError {
    calc(query)
        .result
        .expect_err(&format!("`{query}` gave a value"))
}

fn rates() -> Rates {
    Rates::new(
        "2026-11-25",
        &[("USD", "1.0512"), ("JPY", "161.43"), ("GBP", "0.83125")],
    )
    .unwrap()
}

// R8 acceptance

#[test]
fn sum_of_decimals_shows_the_exact_result() {
    // Arrange
    let query = "42.3+68.7+14.1+0.5";

    // Act
    let result = calc(query);

    // Assert
    assert_eq!(result.text(), "125.6");
}

#[test]
fn enter_copies_the_exact_result_of_the_sum() {
    // Arrange
    let query = "42.3+68.7+14.1+0.5";

    // Act
    let result = calc(query);

    // Assert
    assert_eq!(result.copy_text(), Some("125.6"));
}

#[test]
fn decimal_fractions_add_without_binary_error() {
    assert_eq!(text("0.1+0.2"), "0.3");
    assert_eq!(text("0.1*3"), "0.3");
    assert_eq!(text("1-0.9"), "0.1");
}

// Which text is a calculation

#[test]
fn text_that_is_not_a_calculation_gives_no_row() {
    let queries = [
        "",
        "   ",
        "firefox",
        "e",
        "pi",
        "-pi",
        "d launch",
        "2024-11-24",
        "24.11.2024",
        "11/24/2024",
        "24/11/2024",
        "040-123-4567",
        "~/a/b",
        "/usr/bin",
        "spur 123",
        "f palette-zoom",
        "42",
        "-42",
        "+358401234567",
        "(42)",
        "1e3",
        "3.14",
        "5 GiB",
        "100 usd",
        "10,5",
        "1,000+1",
        "1.2.3",
        "192.168.1.1",
        "17:00 in las vegas",
        "time now in new york",
        "6am in helsinki",
        "sqrt",
        "sqrt(",
        "foo(2)",
        "2pi",
        "c++",
        "5 kg in parsec",
        "100 xyz in eur",
        "3 of 240",
        "1+2)",
        "e-mail",
    ];

    for query in queries {
        assert!(!is_calculation(query), "`{query}` is a calculation");
        assert_eq!(calculate(query, None), None, "`{query}` gave a row");
    }
}

#[test]
fn text_with_an_operation_is_a_calculation() {
    let queries = [
        "42+1",
        "2*pi",
        "-pi*2",
        "sqrt(2)",
        "15%",
        "15% of 240",
        "5 GiB in MB",
        "100 usd in eur",
        "1/0",
        "5 kg in MB",
        "10 - 5 - 2",
        "1/2/3",
        "2024-11",
    ];

    for query in queries {
        assert!(is_calculation(query), "`{query}` is not a calculation");
    }
}

#[test]
fn trailing_operator_is_ignored_while_the_owner_types() {
    assert_eq!(text("42+1+"), "43");
    assert_eq!(text("2*(3+"), "6");
}

#[test]
fn trailing_operator_after_a_bare_value_gives_no_row() {
    for query in ["42+", "42*", "e-", "2024-", "pi^"] {
        assert!(!is_calculation(query), "`{query}` is a calculation");
    }
}

#[test]
fn missing_closing_brackets_are_added_at_the_end() {
    assert_eq!(text("(1+2"), "3");
    assert_eq!(text("sqrt(16"), "4");
    assert_eq!(text("2*(3+(4"), "14");
}

#[test]
fn deep_brackets_give_no_row_and_no_crash() {
    // Arrange
    let query = format!("{}1{}", "(".repeat(10_000), ")".repeat(10_000));

    // Act
    let result = calculate(&query, None);

    // Assert
    assert_eq!(result, None);
}

// Operators

#[test]
fn operators_obey_the_usual_precedence() {
    assert_eq!(text("1+2*3"), "7");
    assert_eq!(text("(1+2)*3"), "9");
    assert_eq!(text("10 - 5 - 2"), "3");
    assert_eq!(text("8/2/2"), "2");
    assert_eq!(text("10/4"), "2.5");
    assert_eq!(text("-3+5"), "2");
    assert_eq!(text("2*-3"), "-6");
}

#[test]
fn power_is_right_associative_and_binds_tighter_than_minus() {
    assert_eq!(text("2^10"), "1024");
    assert_eq!(text("2**10"), "1024");
    assert_eq!(text("-2^2"), "-4");
    assert_eq!(text("2^-2"), "0.25");
    assert_eq!(text("2^3^2"), "512");
    assert_eq!(text("0^0"), "1");
}

#[test]
fn power_with_a_fraction_exponent_is_rounded() {
    assert_eq!(text("2^0.5"), "1.4142135623731");
    assert_eq!(text("1.0825^12"), "2.58901675079278");
}

#[test]
fn mod_gives_a_result_with_the_sign_of_the_divisor() {
    assert_eq!(text("7 mod 3"), "1");
    assert_eq!(text("-7 mod 3"), "2");
    assert_eq!(text("7 mod -3"), "-2");
    assert_eq!(text("7.5 mod 2"), "1.5");
}

#[test]
fn typographic_operators_are_accepted() {
    assert_eq!(text("6×7"), "42");
    assert_eq!(text("8÷2"), "4");
    assert_eq!(text("5−3"), "2");
}

#[test]
fn numbers_accept_an_exponent_and_a_leading_point() {
    assert_eq!(text("1.5e3+1"), "1501");
    assert_eq!(text(".5+1"), "1.5");
    assert_eq!(text("2E-3*1"), "0.002");
}

#[test]
fn exact_results_keep_all_digits() {
    assert_eq!(text("123456789.123456789+1"), "123456790.123456789");
    assert_eq!(text("1 GB in GiB"), "0.931322574615478515625 GiB");
}

#[test]
fn inexact_results_are_rounded_to_15_significant_digits() {
    assert_eq!(text("1/3"), "0.333333333333333");
    assert_eq!(text("2/3"), "0.666666666666667");
    assert_eq!(text("100000/3"), "33333.3333333333");
    assert_eq!(text("10^20/3"), "33333333333333300000");
}

#[test]
fn inexact_results_hide_the_rounding_noise() {
    assert_eq!(text("1/3*3"), "1");
    assert_eq!(text("1/3*3-1"), "0");
    assert_eq!(text("sqrt(2)^2"), "2");
    assert_eq!(text("asin(1)*2-pi"), "0");
}

#[test]
fn division_by_zero_gives_an_error_row() {
    assert_eq!(error("1/0"), CalcError::DivisionByZero);
    assert_eq!(error("5 mod 0"), CalcError::DivisionByZero);
    assert_eq!(error("0^-1"), CalcError::DivisionByZero);
    assert_eq!(calc("1/0").copy_text(), None);
    assert_eq!(calc("1/0").text(), "Division by zero");
}

#[test]
fn too_large_result_gives_an_error_row() {
    assert_eq!(error("10^30"), CalcError::TooLarge);
    assert_eq!(
        error("99999999999999999999999999999*10"),
        CalcError::TooLarge
    );
    assert_eq!(error("1e40*1"), CalcError::TooLarge);
    assert_eq!(error("exp(100)"), CalcError::TooLarge);
}

// Functions

#[test]
fn roots_and_logarithms_give_the_expected_values() {
    assert_eq!(text("sqrt(16)"), "4");
    assert_eq!(text("sqrt(2)"), "1.4142135623731");
    assert_eq!(text("ln(e)"), "1");
    assert_eq!(text("log(1000)"), "3");
    assert_eq!(text("log(8, 2)"), "3");
    assert_eq!(text("log10(0.01)"), "-2");
    assert_eq!(text("log2(1024)"), "10");
    assert_eq!(text("exp(1)"), "2.71828182845905");
    assert_eq!(text("exp(-100)"), "0");
}

#[test]
fn trigonometric_functions_use_radians() {
    assert_eq!(text("sin(pi)"), "0");
    assert_eq!(text("cos(pi)"), "-1");
    assert_eq!(text("sin(pi/6)"), "0.5");
    assert_eq!(text("tan(pi/4)"), "1");
    assert_eq!(text("asin(1)"), "1.5707963267949");
    assert_eq!(text("atan(1)*4"), "3.14159265358979");
    assert_eq!(text("cosh(0)"), "1");
    assert_eq!(text("tanh(0)"), "0");
}

#[test]
fn trigonometric_function_of_a_large_argument_keeps_15_correct_digits() {
    // The reference value is from `bc -l` with 100 decimal places.
    assert_eq!(text("sin(100000000000000)"), "-0.209408307496452");
}

#[test]
fn trigonometric_function_of_a_too_large_argument_gives_an_error_row() {
    assert_eq!(error("sin(1e15)"), CalcError::TooLarge);
    assert_eq!(error("cos(-1e15)"), CalcError::TooLarge);
    assert_eq!(error("tan(1e15)"), CalcError::TooLarge);
}

#[test]
fn rounding_functions_round_half_away_from_zero() {
    assert_eq!(text("round(2.5)"), "3");
    assert_eq!(text("round(-2.5)"), "-3");
    assert_eq!(text("round(3.14159, 2)"), "3.14");
    assert_eq!(text("floor(-2.5)"), "-3");
    assert_eq!(text("ceil(2.1)"), "3");
    assert_eq!(text("abs(-3.5)"), "3.5");
}

#[test]
fn function_names_ignore_case() {
    assert_eq!(text("SQRT(4)"), "2");
    assert_eq!(text("2*PI"), "6.28318530717959");
}

#[test]
fn value_outside_the_domain_gives_an_error_row() {
    assert!(matches!(error("sqrt(-1)"), CalcError::Undefined(_)));
    assert!(matches!(error("ln(0)"), CalcError::Undefined(_)));
    assert!(matches!(error("acos(2)"), CalcError::Undefined(_)));
    assert!(matches!(error("(-8)^0.5"), CalcError::Undefined(_)));
    assert!(matches!(error("log(8, 1)"), CalcError::Undefined(_)));
}

#[test]
fn round_with_bad_digits_gives_an_error_row() {
    assert!(matches!(
        error("round(1, 1.5)"),
        CalcError::InvalidArgument(_)
    ));
    assert!(matches!(
        error("round(1, 40)"),
        CalcError::InvalidArgument(_)
    ));
}

#[test]
fn function_with_the_wrong_number_of_arguments_gives_no_row() {
    assert!(!is_calculation("sqrt(1, 2)"));
    assert!(!is_calculation("log(1, 2, 3)"));
    assert!(!is_calculation("sqrt()"));
}

// Percent (R15)

#[test]
fn percent_of_a_value() {
    assert_eq!(text("15% of 240"), "36");
    assert_eq!(text("15 % OF 240"), "36");
    assert_eq!(text("(10+5)% of 240"), "36");
    assert_eq!(text("50% of 50% of 240"), "60");
}

#[test]
fn added_or_subtracted_percent_is_relative_to_the_left_value() {
    assert_eq!(text("240 + 15%"), "276");
    assert_eq!(text("240 - 15%"), "204");
    assert_eq!(text("240 + (15%)"), "276");
}

#[test]
fn percent_in_other_places_is_a_hundredth() {
    assert_eq!(text("15%"), "0.15");
    assert_eq!(text("240 * 15%"), "36");
    assert_eq!(text("240 / 50%"), "480");
    assert_eq!(text("1 + 15% * 2"), "1.3");
}

// Units (R15)

#[test]
fn data_size_conversion_gives_the_exact_result() {
    // Arrange
    let query = "5 GiB in MB";

    // Act
    let result = calc(query);

    // Assert
    assert_eq!(result.text(), "5368.70912 MB");
    assert_eq!(result.copy_text(), Some("5368.70912"));
}

#[test]
fn unit_names_accept_symbols_names_plurals_and_any_case() {
    assert_eq!(text("5 gib to mb"), "5368.70912 MB");
    assert_eq!(text("5GiB in MB"), "5368.70912 MB");
    assert_eq!(text("2 gibibytes in megabytes"), "2147.483648 MB");
    assert_eq!(text("1 KiB in B"), "1024 B");
    assert_eq!(text("1 byte in bits"), "8 bit");
    assert_eq!(text("1 Gbit in MB"), "125 MB");
}

#[test]
fn time_units_convert() {
    assert_eq!(text("90 min in h"), "1.5 h");
    assert_eq!(text("1 week in hours"), "168 h");
    assert_eq!(text("1 day in minutes"), "1440 min");
    assert_eq!(text("1500 ms in s"), "1.5 s");
    assert_eq!(text("1 s in wk"), "0.000001653439153 wk");
}

#[test]
fn length_units_convert() {
    assert_eq!(text("1 mi in km"), "1.609344 km");
    assert_eq!(text("1 ft in in"), "12 in");
    assert_eq!(text("5 in in cm"), "12.7 cm");
    assert_eq!(text("1 m in ft"), "3.28083989501312 ft");
    assert_eq!(text("100 metres in yards"), "109.361329833771 yd");
}

#[test]
fn mass_units_convert() {
    assert_eq!(text("1 lb in kg"), "0.45359237 kg");
    assert_eq!(text("1 kg in lb"), "2.20462262184878 lb");
    assert_eq!(text("1 st in lb"), "14 lb");
    assert_eq!(text("16 oz in lb"), "1 lb");
    assert_eq!(text("2.5 t in kg"), "2500 kg");
}

#[test]
fn temperature_units_convert_with_the_offset() {
    assert_eq!(text("100 C in F"), "212 °F");
    assert_eq!(text("-40 °C in °F"), "-40 °F");
    assert_eq!(text("0 celsius in kelvin"), "273.15 K");
    assert_eq!(text("212 F in C"), "100 °C");
    assert_eq!(text("100 F in C"), "37.7777777777778 °C");
    assert_eq!(text("0 K in F"), "-459.67 °F");
}

#[test]
fn unit_value_can_be_an_expression() {
    assert_eq!(text("(1+2) kg in g"), "3000 g");
    assert_eq!(text("2^10 KiB in MiB"), "1 MiB");
    assert_eq!(text("15% of 240 GiB in MiB"), "36864 MiB");
}

#[test]
fn conversion_between_dimensions_gives_an_error_row() {
    // Arrange
    let query = "5 kg in MB";

    // Act
    let result = calc(query);

    // Assert
    assert_eq!(
        result.result,
        Err(CalcError::IncompatibleUnits {
            from: Dimension::Mass,
            to: Dimension::DataSize
        })
    );
    assert_eq!(result.text(), "Cannot convert mass to data size");
    assert_eq!(result.copy_text(), None);
}

// Currencies (R15)

#[test]
fn currency_converts_through_eur_and_shows_the_date_of_the_rates() {
    // Arrange
    let rates = rates();

    // Act
    let result = calculate("100 usd in eur", Some(&rates)).unwrap();

    // Assert
    assert_eq!(result.text(), "95.13 EUR");
    assert_eq!(result.copy_text(), Some("95.13"));
    assert!(result.note.unwrap().contains("2026-11-25"));
}

#[test]
fn currency_converts_between_two_currencies_other_than_eur() {
    let rates = rates();

    assert_eq!(
        calculate("100 USD to GBP", Some(&rates)).unwrap().text(),
        "79.08 GBP"
    );
    assert_eq!(
        calculate("100 eur in usd", Some(&rates)).unwrap().text(),
        "105.12 USD"
    );
}

#[test]
fn small_currency_amount_keeps_four_significant_digits() {
    let rates = rates();

    assert_eq!(
        calculate("1 jpy in eur", Some(&rates)).unwrap().text(),
        "0.006195 EUR"
    );
}

#[test]
fn currency_without_rates_gives_a_no_rates_row() {
    // Act
    let result = calculate("100 usd in eur", None).unwrap();

    // Assert
    assert_eq!(result.result, Err(CalcError::NoRates));
    assert_eq!(result.text(), "No currency rates available");
    assert_eq!(result.copy_text(), None);
}

#[test]
fn currency_that_is_not_in_the_rates_gives_an_error_row() {
    let rates = rates();

    let result = calculate("100 sek in eur", Some(&rates)).unwrap();

    assert_eq!(result.result, Err(CalcError::MissingRate("SEK".into())));
}

#[test]
fn currency_and_unit_do_not_convert() {
    let rates = rates();

    let result = calculate("100 usd in kg", Some(&rates)).unwrap();

    assert_eq!(
        result.result,
        Err(CalcError::IncompatibleUnits {
            from: Dimension::Currency,
            to: Dimension::Mass
        })
    );
}

// Robustness

#[test]
fn generated_queries_do_not_panic() {
    // Arrange
    let pieces = [
        "1",
        "0",
        "2.5",
        "1e3",
        "99999999999999999999999999",
        "0.0000000001",
        "+",
        "-",
        "*",
        "/",
        "^",
        "%",
        "(",
        ")",
        ",",
        " ",
        "mod",
        "of",
        "in",
        "to",
        "pi",
        "e",
        "sqrt",
        "ln",
        "log",
        "sin",
        "tan",
        "asin",
        "exp",
        "round",
        "kg",
        "GiB",
        "C",
        "F",
        "usd",
        "eur",
        "x",
    ];
    let rates = rates();
    let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    // Act and assert: a panic fails the test
    for _ in 0..20_000 {
        let length = next() % 9 + 1;
        let query: String = (0..length)
            .map(|_| pieces[(next() % pieces.len() as u64) as usize])
            .collect();
        let _ = calculate(&query, Some(&rates));
        let _ = calculate(&query, None);
    }
}
