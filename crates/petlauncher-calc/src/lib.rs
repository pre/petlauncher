//! Calculator of the petlauncher query line: arithmetic, percent, units and
//! currencies (R8, R15).
//!
//! [`calculate`] gives `None` when the query is not a calculation, and the
//! launcher shows no calculation row. Each [`Calculation`] is a calculation
//! row, also when it holds an error. The syntax, the rule that decides if a
//! text is a calculation, the units and the rounding are in
//! `docs/calculator.md`.
//!
//! The evaluator has no network code. The caller gives the currency rates as
//! a [`Rates`] value. The functions in [`rates`] read the ECB file, keep it in
//! a cache directory and, with the cargo feature `fetch`, download it.

use std::fmt;

mod eval;
mod functions;
mod lexer;
mod number;
mod parser;
pub mod rates;
mod units;

#[cfg(feature = "fetch")]
pub use rates::{fetch_ecb, update_cache};
pub use rates::{load_cache, parse_ecb, save_cache, Rates, RatesError, CACHE_FILE, ECB_DAILY_URL};
pub use units::Dimension;

/// The result of a query that is a calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calculation {
    pub result: Result<Answer, CalcError>,
    /// Text for the subtitle of the row, for example the date of the
    /// currency rates.
    pub note: Option<String>,
}

/// A calculated value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    /// The number with `.` as the decimal mark and no group separators, for
    /// example `5368.70912`.
    pub number: String,
    /// The symbol of the target unit or the currency code of a conversion,
    /// for example `MB` or `EUR`.
    pub unit: Option<String>,
}

impl Calculation {
    /// The title of the row: `125.6`, `5368.70912 MB`, or the error message.
    pub fn text(&self) -> String {
        match &self.result {
            Ok(Answer {
                number,
                unit: Some(unit),
            }) => format!("{number} {unit}"),
            Ok(Answer { number, unit: None }) => number.clone(),
            Err(error) => error.to_string(),
        }
    }

    /// The text that Enter copies: the number only, so that another program
    /// can read it. `None` for an error.
    pub fn copy_text(&self) -> Option<&str> {
        self.result
            .as_ref()
            .ok()
            .map(|answer| answer.number.as_str())
    }
}

/// Why a calculation has no value. The row shows the message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalcError {
    DivisionByZero,
    /// The value is out of the range of the decimal type, about ±7.9e28.
    TooLarge,
    /// The function has no real result for the argument, for example
    /// `sqrt(-1)` or `ln(0)`. The text is the function name.
    Undefined(&'static str),
    InvalidArgument(&'static str),
    IncompatibleUnits {
        from: Dimension,
        to: Dimension,
    },
    /// A currency conversion, and the caller has no rates.
    NoRates,
    /// The rates have no rate for this currency code.
    MissingRate(String),
}

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CalcError::DivisionByZero => f.write_str("Division by zero"),
            CalcError::TooLarge => f.write_str("The number is too large"),
            CalcError::Undefined(function) => write!(f, "No real result for {function}"),
            CalcError::InvalidArgument(message) => f.write_str(message),
            CalcError::IncompatibleUnits { from, to } => {
                write!(f, "Cannot convert {from} to {to}")
            }
            CalcError::NoRates => f.write_str("No currency rates available"),
            CalcError::MissingRate(code) => write!(f, "No rate for {code}"),
        }
    }
}

impl std::error::Error for CalcError {}

/// True when the query is a calculation, so that the launcher shows a
/// calculation row for it. Needs no rates.
pub fn is_calculation(query: &str) -> bool {
    parser::parse(query).is_some()
}

/// Calculates the query. `None` when the query is not a calculation.
/// `rates` is necessary only for a currency conversion.
pub fn calculate(query: &str, rates: Option<&Rates>) -> Option<Calculation> {
    let query = parser::parse(query)?;
    Some(eval::evaluate(&query, rates))
}
