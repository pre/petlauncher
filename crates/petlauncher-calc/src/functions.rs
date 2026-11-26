//! Constants and functions of the calculator.

use std::ops::RangeInclusive;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps, RoundingStrategy};

use crate::number::Num;
use crate::CalcError;

/// The largest number of decimal places that `round(x, n)` accepts.
const MAX_ROUND_PLACES: u32 = 28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Constant {
    Pi,
    E,
}

impl Constant {
    pub fn from_name(name: &str) -> Option<Constant> {
        match name.to_lowercase().as_str() {
            "pi" | "π" => Some(Constant::Pi),
            "e" => Some(Constant::E),
            _ => None,
        }
    }

    /// 28 decimal places, so the value is not exact.
    pub fn value(self) -> Num {
        Num::inexact(match self {
            Constant::Pi => Decimal::PI,
            Constant::E => Decimal::E,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Function {
    Sqrt,
    Exp,
    Ln,
    Log,
    Log10,
    Log2,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Asinh,
    Acosh,
    Atanh,
    Abs,
    Round,
    Floor,
    Ceil,
}

impl Function {
    pub fn from_name(name: &str) -> Option<Function> {
        use Function::*;
        Some(match name.to_lowercase().as_str() {
            "sqrt" => Sqrt,
            "exp" => Exp,
            "ln" => Ln,
            "log" => Log,
            "log10" => Log10,
            "log2" => Log2,
            "sin" => Sin,
            "cos" => Cos,
            "tan" => Tan,
            "asin" => Asin,
            "acos" => Acos,
            "atan" => Atan,
            "sinh" => Sinh,
            "cosh" => Cosh,
            "tanh" => Tanh,
            "asinh" => Asinh,
            "acosh" => Acosh,
            "atanh" => Atanh,
            "abs" => Abs,
            "round" => Round,
            "floor" => Floor,
            "ceil" => Ceil,
            _ => return None,
        })
    }

    /// The accepted number of arguments.
    pub fn arity(self) -> RangeInclusive<usize> {
        match self {
            Function::Log | Function::Round => 1..=2,
            _ => 1..=1,
        }
    }

    pub fn apply(self, arguments: &[Num]) -> Result<Num, CalcError> {
        use Function::*;
        let x = arguments[0];
        match self {
            Sqrt => sqrt(x),
            Exp => exp(x),
            Ln => ln(x, "ln"),
            Log => match arguments.get(1) {
                None => log10(x),
                Some(&base) => {
                    if base.is_negative() || base.is_zero() || base.value == Decimal::ONE {
                        return Err(CalcError::Undefined("log"));
                    }
                    ln(x, "log")?.div(ln(base, "log")?)
                }
            },
            Log10 => log10(x),
            Log2 => ln(x, "log2")?.div(ln(Num::from_int(2), "log2")?),
            Sin => trigonometric(x, Decimal::checked_sin, "sin"),
            Cos => trigonometric(x, Decimal::checked_cos, "cos"),
            // The tangent of the decimal library has only about 8 correct digits.
            Tan => {
                let cosine = trigonometric(x, Decimal::checked_cos, "tan")?;
                if cosine.is_zero() {
                    return Err(CalcError::Undefined("tan"));
                }
                trigonometric(x, Decimal::checked_sin, "tan")?.div(cosine)
            }
            Asin => float(x, f64::asin, "asin"),
            Acos => float(x, f64::acos, "acos"),
            Atan => float(x, f64::atan, "atan"),
            Sinh => float(x, f64::sinh, "sinh"),
            Cosh => float(x, f64::cosh, "cosh"),
            Tanh => float(x, f64::tanh, "tanh"),
            Asinh => float(x, f64::asinh, "asinh"),
            Acosh => float(x, f64::acosh, "acosh"),
            Atanh => float(x, f64::atanh, "atanh"),
            Abs => Ok(Num {
                value: x.value.abs(),
                ..x
            }),
            Round => {
                let places = match arguments.get(1) {
                    None => 0,
                    Some(places) => round_places(*places)?,
                };
                Ok(Num {
                    value: x
                        .value
                        .round_dp_with_strategy(places, RoundingStrategy::MidpointAwayFromZero),
                    ..x
                })
            }
            Floor => Ok(Num {
                value: x.value.floor(),
                ..x
            }),
            Ceil => Ok(Num {
                value: x.value.ceil(),
                ..x
            }),
        }
    }
}

fn sqrt(x: Num) -> Result<Num, CalcError> {
    if x.is_negative() {
        return Err(CalcError::Undefined("sqrt"));
    }
    let root = x.value.sqrt().ok_or(CalcError::Undefined("sqrt"))?;
    let exact = x.exact
        && Num::exact(root)
            .mul(Num::exact(root))
            .is_ok_and(|square| square.exact && square.value == x.value);
    Ok(Num { value: root, exact })
}

fn exp(x: Num) -> Result<Num, CalcError> {
    match x.value.checked_exp() {
        Some(value) => Ok(Num::inexact(value)),
        // Below the smallest decimal value.
        None if x.is_negative() => Ok(Num::inexact(Decimal::ZERO)),
        None => Err(CalcError::TooLarge),
    }
}

fn ln(x: Num, name: &'static str) -> Result<Num, CalcError> {
    if x.is_negative() || x.is_zero() {
        return Err(CalcError::Undefined(name));
    }
    decimal(x.value.checked_ln(), name)
}

fn log10(x: Num) -> Result<Num, CalcError> {
    if x.is_negative() || x.is_zero() {
        return Err(CalcError::Undefined("log"));
    }
    decimal(x.value.checked_log10(), "log")
}

/// For a larger argument, the decimal sine and cosine have fewer than 15
/// correct digits. A check against `bc -l` gave correct digits for 1e14 and
/// an error in the 12th digit for 1e16.
const MAX_TRIGONOMETRIC_ARGUMENT: i64 = 100_000_000_000_000;

fn trigonometric(
    x: Num,
    function: fn(&Decimal) -> Option<Decimal>,
    name: &'static str,
) -> Result<Num, CalcError> {
    if x.value.abs() > Decimal::from(MAX_TRIGONOMETRIC_ARGUMENT) {
        return Err(CalcError::TooLarge);
    }
    decimal(function(&x.value), name)
}

fn decimal(value: Option<Decimal>, name: &'static str) -> Result<Num, CalcError> {
    value.map(Num::inexact).ok_or(CalcError::Undefined(name))
}

/// For the functions that the decimal library does not have. A binary
/// float has about 16 significant digits, and a result shows 15.
/// `from_f64_retain` keeps the exact binary value. `from_f64` rounds it to
/// 16 digits, and that error shows in `asin(1)*2-pi`.
fn float(x: Num, function: fn(f64) -> f64, name: &'static str) -> Result<Num, CalcError> {
    let input = x.value.to_f64().ok_or(CalcError::TooLarge)?;
    let output = function(input);
    if output.is_nan() {
        return Err(CalcError::Undefined(name));
    }
    Decimal::from_f64_retain(output)
        .map(Num::inexact)
        .ok_or(CalcError::TooLarge)
}

fn round_places(places: Num) -> Result<u32, CalcError> {
    places
        .value
        .is_integer()
        .then(|| places.value.to_u32())
        .flatten()
        .filter(|places| *places <= MAX_ROUND_PLACES)
        .ok_or(CalcError::InvalidArgument(
            "The places of round must be a whole number from 0 to 28",
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_square_has_an_exact_root() {
        let root = sqrt(Num::from_int(16)).unwrap();

        assert!(root.exact);
    }

    #[test]
    fn root_of_two_is_not_exact() {
        let root = sqrt(Num::from_int(2)).unwrap();

        assert!(!root.exact);
    }

    #[test]
    fn infinite_float_result_is_too_large() {
        let result = Function::Sinh.apply(&[Num::from_int(1000)]);

        assert_eq!(result, Err(CalcError::TooLarge));
    }
}
