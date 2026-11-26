//! Units, currency codes and unit conversions.

use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;

use crate::number::Num;
use crate::CalcError;

/// The physical quantity of a unit. Only units with the same dimension convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    DataSize,
    Time,
    Length,
    Mass,
    Temperature,
    Currency,
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Dimension::DataSize => "data size",
            Dimension::Time => "time",
            Dimension::Length => "length",
            Dimension::Mass => "mass",
            Dimension::Temperature => "temperature",
            Dimension::Currency => "currency",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scale {
    /// The size of the unit in the base unit of its dimension: bit, second,
    /// metre or kilogram. The text is an exact decimal.
    Factor(&'static str),
    Celsius,
    Fahrenheit,
    Kelvin,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Unit {
    /// The name that the result shows.
    pub symbol: &'static str,
    /// All accepted names, the symbol included. Case is ignored.
    names: &'static [&'static str],
    pub dimension: Dimension,
    scale: Scale,
}

const fn unit(
    symbol: &'static str,
    dimension: Dimension,
    scale: Scale,
    names: &'static [&'static str],
) -> Unit {
    Unit {
        symbol,
        names,
        dimension,
        scale,
    }
}

use Dimension::{DataSize, Length, Mass, Temperature, Time};
use Scale::{Celsius, Factor, Fahrenheit, Kelvin};

#[rustfmt::skip]
static UNITS: &[Unit] = &[
    unit("bit",  DataSize, Factor("1"),                &["bit", "bits"]),
    unit("kbit", DataSize, Factor("1000"),             &["kbit", "kilobit", "kilobits"]),
    unit("Mbit", DataSize, Factor("1000000"),          &["Mbit", "megabit", "megabits"]),
    unit("Gbit", DataSize, Factor("1000000000"),       &["Gbit", "gigabit", "gigabits"]),
    unit("Tbit", DataSize, Factor("1000000000000"),    &["Tbit", "terabit", "terabits"]),
    unit("B",    DataSize, Factor("8"),                &["B", "byte", "bytes"]),
    unit("kB",   DataSize, Factor("8000"),             &["kB", "kilobyte", "kilobytes"]),
    unit("MB",   DataSize, Factor("8000000"),          &["MB", "megabyte", "megabytes"]),
    unit("GB",   DataSize, Factor("8000000000"),       &["GB", "gigabyte", "gigabytes"]),
    unit("TB",   DataSize, Factor("8000000000000"),    &["TB", "terabyte", "terabytes"]),
    unit("PB",   DataSize, Factor("8000000000000000"), &["PB", "petabyte", "petabytes"]),
    unit("KiB",  DataSize, Factor("8192"),             &["KiB", "kibibyte", "kibibytes"]),
    unit("MiB",  DataSize, Factor("8388608"),          &["MiB", "mebibyte", "mebibytes"]),
    unit("GiB",  DataSize, Factor("8589934592"),       &["GiB", "gibibyte", "gibibytes"]),
    unit("TiB",  DataSize, Factor("8796093022208"),    &["TiB", "tebibyte", "tebibytes"]),
    unit("PiB",  DataSize, Factor("9007199254740992"), &["PiB", "pebibyte", "pebibytes"]),

    unit("ms",  Time, Factor("0.001"),  &["ms", "millisecond", "milliseconds"]),
    unit("s",   Time, Factor("1"),      &["s", "sec", "secs", "second", "seconds"]),
    unit("min", Time, Factor("60"),     &["min", "mins", "minute", "minutes"]),
    unit("h",   Time, Factor("3600"),   &["h", "hr", "hrs", "hour", "hours"]),
    unit("d",   Time, Factor("86400"),  &["d", "day", "days"]),
    unit("wk",  Time, Factor("604800"), &["wk", "wks", "week", "weeks"]),

    unit("mm",  Length, Factor("0.001"),    &["mm", "millimeter", "millimeters", "millimetre", "millimetres"]),
    unit("cm",  Length, Factor("0.01"),     &["cm", "centimeter", "centimeters", "centimetre", "centimetres"]),
    unit("m",   Length, Factor("1"),        &["m", "meter", "meters", "metre", "metres"]),
    unit("km",  Length, Factor("1000"),     &["km", "kilometer", "kilometers", "kilometre", "kilometres"]),
    unit("in",  Length, Factor("0.0254"),   &["in", "inch", "inches"]),
    unit("ft",  Length, Factor("0.3048"),   &["ft", "foot", "feet"]),
    unit("yd",  Length, Factor("0.9144"),   &["yd", "yard", "yards"]),
    unit("mi",  Length, Factor("1609.344"), &["mi", "mile", "miles"]),
    unit("nmi", Length, Factor("1852"),     &["nmi"]),

    unit("mg", Mass, Factor("0.000001"),       &["mg", "milligram", "milligrams"]),
    unit("g",  Mass, Factor("0.001"),          &["g", "gram", "grams"]),
    unit("kg", Mass, Factor("1"),              &["kg", "kilogram", "kilograms", "kilo", "kilos"]),
    unit("t",  Mass, Factor("1000"),           &["t", "tonne", "tonnes"]),
    unit("oz", Mass, Factor("0.028349523125"), &["oz", "ounce", "ounces"]),
    unit("lb", Mass, Factor("0.45359237"),     &["lb", "lbs", "pound", "pounds"]),
    unit("st", Mass, Factor("6.35029318"),     &["st", "stone", "stones"]),

    unit("°C", Temperature, Celsius,    &["°C", "C", "degC", "celsius"]),
    unit("°F", Temperature, Fahrenheit, &["°F", "F", "degF", "fahrenheit"]),
    unit("K",  Temperature, Kelvin,     &["K", "kelvin", "kelvins"]),
];

/// ISO 4217 codes of EUR and of the currencies in the ECB daily reference
/// rates. A code in this list is a currency also when no rates are stored.
static CURRENCIES: &[&str] = &[
    "EUR", "USD", "JPY", "CZK", "DKK", "GBP", "HUF", "PLN", "RON", "SEK", "CHF", "ISK", "NOK",
    "TRY", "AUD", "BRL", "CAD", "CNY", "HKD", "IDR", "ILS", "INR", "KRW", "MXN", "MYR", "NZD",
    "PHP", "SGD", "THB", "ZAR",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnitRef {
    Unit(&'static Unit),
    /// An upper case ISO 4217 code.
    Currency(&'static str),
}

impl UnitRef {
    pub fn dimension(self) -> Dimension {
        match self {
            UnitRef::Unit(unit) => unit.dimension,
            UnitRef::Currency(_) => Dimension::Currency,
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            UnitRef::Unit(unit) => unit.symbol,
            UnitRef::Currency(code) => code,
        }
    }
}

pub(crate) fn lookup(name: &str) -> Option<UnitRef> {
    let name = name.to_lowercase();
    let unit = UNITS
        .iter()
        .find(|unit| unit.names.iter().any(|n| n.to_lowercase() == name));
    if let Some(unit) = unit {
        return Some(UnitRef::Unit(unit));
    }
    CURRENCIES
        .iter()
        .find(|code| code.to_lowercase() == name)
        .map(|code| UnitRef::Currency(code))
}

/// Converts between two units of the same dimension. The caller converts currencies.
pub(crate) fn convert(value: Num, from: &Unit, to: &Unit) -> Result<Num, CalcError> {
    if from.dimension != to.dimension {
        return Err(CalcError::IncompatibleUnits {
            from: from.dimension,
            to: to.dimension,
        });
    }
    match (from.scale, to.scale) {
        (Factor(from), Factor(to)) => value.mul(factor(from))?.div(factor(to)),
        (from, to) => from_celsius(to_celsius(value, from)?, to),
    }
}

fn factor(text: &str) -> Num {
    Num::exact(Decimal::from_str(text).expect("the unit table has valid numbers"))
}

fn to_celsius(value: Num, scale: Scale) -> Result<Num, CalcError> {
    match scale {
        Fahrenheit => value.sub(factor("32"))?.div(factor("1.8")),
        Kelvin => value.sub(factor("273.15")),
        _ => Ok(value),
    }
}

fn from_celsius(value: Num, scale: Scale) -> Result<Num, CalcError> {
    match scale {
        Fahrenheit => value.mul(factor("1.8"))?.add(factor("32")),
        Kelvin => value.add(factor("273.15")),
        _ => Ok(value),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn each_name_has_one_meaning_when_case_is_ignored() {
        let mut seen = HashSet::new();
        let names = UNITS
            .iter()
            .flat_map(|unit| unit.names.iter().copied())
            .chain(CURRENCIES.iter().copied());

        for name in names {
            assert!(seen.insert(name.to_lowercase()), "`{name}` is a duplicate");
        }
    }

    #[test]
    fn each_unit_accepts_its_symbol() {
        for unit in UNITS {
            assert_eq!(lookup(unit.symbol), Some(UnitRef::Unit(unit)));
        }
    }

    #[test]
    fn each_factor_is_a_positive_decimal() {
        for unit in UNITS {
            if let Factor(text) = unit.scale {
                assert!(factor(text).value > Decimal::ZERO, "{}", unit.symbol);
            }
        }
    }

    #[test]
    fn currency_code_ignores_case() {
        assert_eq!(lookup("usd"), Some(UnitRef::Currency("USD")));
    }

    #[test]
    fn unknown_name_is_not_a_unit() {
        assert_eq!(lookup("parsec"), None);
        assert_eq!(lookup("xyz"), None);
    }
}
