//! A decimal number that knows if it is exact, and the rules that make its text.

use std::str::FromStr;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, MathematicalOps, RoundingStrategy};

use crate::CalcError;

/// Significant digits of an inexact result.
const SHOWN_DIGITS: u32 = 15;
/// Decimal places of an inexact result below 1. This removes the rounding
/// noise of the last digits, for example in `1/3*3-1`.
const SHOWN_PLACES: u32 = 15;
const ROUNDING: RoundingStrategy = RoundingStrategy::MidpointAwayFromZero;

/// `exact` is false when the value is rounded: a division with a remainder,
/// a root, a logarithm, a constant, or a result with too many digits.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Num {
    pub value: Decimal,
    pub exact: bool,
}

impl Num {
    pub fn exact(value: Decimal) -> Self {
        Num { value, exact: true }
    }

    pub fn inexact(value: Decimal) -> Self {
        Num {
            value,
            exact: false,
        }
    }

    pub fn from_int(value: i64) -> Self {
        Num::exact(Decimal::from(value))
    }

    /// Reads a literal from the lexer. A literal with more digits than the
    /// decimal type holds is rounded and is not exact.
    pub fn parse(literal: &str) -> Result<Num, CalcError> {
        if literal.contains('e') {
            return match Decimal::from_scientific(literal) {
                Ok(value) => Ok(Num::exact(value)),
                Err(_) => Decimal::from_scientific_lossy(literal)
                    .map(Num::inexact)
                    .map_err(|_| CalcError::TooLarge),
            };
        }
        match Decimal::from_str_exact(literal) {
            Ok(value) => Ok(Num::exact(value)),
            Err(_) => Decimal::from_str(literal)
                .map(Num::inexact)
                .map_err(|_| CalcError::TooLarge),
        }
    }

    pub fn is_zero(self) -> bool {
        self.value.is_zero()
    }

    pub fn is_negative(self) -> bool {
        self.value.is_sign_negative() && !self.value.is_zero()
    }

    pub fn neg(self) -> Num {
        Num {
            value: -self.value,
            ..self
        }
    }

    pub fn add(self, other: Num) -> Result<Num, CalcError> {
        let (a, b) = (self.value, other.value);
        let value = a.checked_add(b).ok_or(CalcError::TooLarge)?;
        Ok(Num {
            value,
            exact: self.exact && other.exact && sum_is_exact(a, b, value),
        })
    }

    pub fn sub(self, other: Num) -> Result<Num, CalcError> {
        self.add(other.neg())
    }

    pub fn mul(self, other: Num) -> Result<Num, CalcError> {
        let (a, b) = (self.value, other.value);
        let value = a.checked_mul(b).ok_or(CalcError::TooLarge)?;
        Ok(Num {
            value,
            exact: self.exact && other.exact && product_is_exact(a, b, value),
        })
    }

    pub fn div(self, other: Num) -> Result<Num, CalcError> {
        if other.is_zero() {
            return Err(CalcError::DivisionByZero);
        }
        let value = self
            .value
            .checked_div(other.value)
            .ok_or(CalcError::TooLarge)?;
        let exact = self.exact
            && other.exact
            && value.checked_mul(other.value).is_some_and(|product| {
                product == self.value && product_is_exact(value, other.value, product)
            });
        Ok(Num { value, exact })
    }

    /// Modulo with the sign of the divisor: `-7 mod 3` is 2.
    pub fn rem(self, other: Num) -> Result<Num, CalcError> {
        if other.is_zero() {
            return Err(CalcError::DivisionByZero);
        }
        let mut value = self
            .value
            .checked_rem(other.value)
            .ok_or(CalcError::TooLarge)?;
        if !value.is_zero() && value.is_sign_negative() != other.value.is_sign_negative() {
            value = value.checked_add(other.value).ok_or(CalcError::TooLarge)?;
        }
        Ok(Num {
            value,
            exact: self.exact && other.exact,
        })
    }

    pub fn pow(self, exponent: Num) -> Result<Num, CalcError> {
        if exponent.value.is_integer() {
            let n = exponent.value.to_i64().ok_or(CalcError::TooLarge)?;
            let power = self.pow_unsigned(n.unsigned_abs())?;
            let power = if n < 0 {
                Num::from_int(1).div(power)?
            } else {
                power
            };
            return Ok(Num {
                exact: power.exact && exponent.exact,
                ..power
            });
        }
        if self.is_negative() {
            return Err(CalcError::Undefined("power"));
        }
        if self.is_zero() {
            return if exponent.is_negative() {
                Err(CalcError::DivisionByZero)
            } else {
                Ok(Num::from_int(0))
            };
        }
        match self.value.checked_powd(exponent.value) {
            Some(value) => Ok(Num::inexact(value)),
            // No value: the result is outside the range. It is near zero
            // when the base and the exponent move it toward zero.
            None if (self.value < Decimal::ONE) == (exponent.value > Decimal::ZERO) => {
                Ok(Num::inexact(Decimal::ZERO))
            }
            None => Err(CalcError::TooLarge),
        }
    }

    /// Power by squaring, so that an exact base gives an exact power.
    fn pow_unsigned(self, mut n: u64) -> Result<Num, CalcError> {
        let mut result = Num::from_int(1);
        let mut base = self;
        while n > 0 {
            if n & 1 == 1 {
                result = result.mul(base)?;
            }
            n >>= 1;
            if n > 0 {
                base = base.mul(base)?;
            }
        }
        Ok(result)
    }

    /// The text of a result. An exact value shows all its digits. An inexact
    /// value is rounded to 15 significant digits and at most 15 decimal places.
    pub fn text(self) -> String {
        let value = if self.exact {
            self.value
        } else if self.value.abs() < Decimal::ONE {
            self.value.round_dp_with_strategy(SHOWN_PLACES, ROUNDING)
        } else {
            self.value
                .round_sf_with_strategy(SHOWN_DIGITS, ROUNDING)
                .unwrap_or(self.value)
        };
        plain(value)
    }
}

// The decimal type rounds a sum or a product that has more than about 28
// digits, and it gives no signal. These checks calculate the exact mantissa
// in i128 and compare. When i128 is too small, the value is not exact.

fn sum_is_exact(a: Decimal, b: Decimal, sum: Decimal) -> bool {
    let (a, b) = (a.normalize(), b.normalize());
    let scale = a.scale().max(b.scale());
    let aligned = |x: Decimal| {
        10i128
            .checked_pow(scale - x.scale())
            .and_then(|power| x.mantissa().checked_mul(power))
    };
    let exact_sum = aligned(a)
        .zip(aligned(b))
        .and_then(|(a, b)| a.checked_add(b));
    exact_sum.is_some_and(|mantissa| equals_exactly(sum, mantissa, scale))
}

fn product_is_exact(a: Decimal, b: Decimal, product: Decimal) -> bool {
    let (a, b) = (a.normalize(), b.normalize());
    a.mantissa()
        .checked_mul(b.mantissa())
        .is_some_and(|mantissa| equals_exactly(product, mantissa, a.scale() + b.scale()))
}

/// True when `value` is `mantissa × 10^-scale`.
fn equals_exactly(value: Decimal, mut mantissa: i128, mut scale: u32) -> bool {
    while scale > 0 && mantissa % 10 == 0 {
        mantissa /= 10;
        scale -= 1;
    }
    let value = value.normalize();
    value.mantissa() == mantissa && value.scale() == scale
}

/// The text of an amount of money: two decimal places, or four significant
/// digits when the amount is less than 1.
pub(crate) fn money_text(value: Decimal) -> String {
    if value.is_zero() {
        return "0.00".into();
    }
    if value.abs() >= Decimal::ONE {
        let mut rounded = value.round_dp_with_strategy(2, ROUNDING);
        rounded.rescale(2);
        rounded.to_string()
    } else {
        plain(value.round_sf_with_strategy(4, ROUNDING).unwrap_or(value))
    }
}

/// Decimal point, no group separators, no exponent, no trailing zeros, no `-0`.
fn plain(value: Decimal) -> String {
    if value.is_zero() {
        return "0".into();
    }
    value.normalize().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(text: &str) -> Num {
        Num::parse(text).unwrap()
    }

    #[test]
    fn sum_with_too_many_digits_is_not_exact() {
        let result = num("10000000000000000000000000000")
            .add(num("0.1"))
            .unwrap();

        assert!(!result.exact);
    }

    #[test]
    fn product_with_too_many_decimal_places_is_not_exact() {
        let result = num("0.123456789012345")
            .mul(num("0.123456789012345"))
            .unwrap();

        assert!(!result.exact);
    }

    #[test]
    fn product_of_values_with_trailing_zeros_stays_exact() {
        let result = num("1.50000000000000000000")
            .mul(num("2.00000000000000000000"))
            .unwrap();

        assert!(result.exact);
        assert_eq!(result.text(), "3");
    }

    #[test]
    fn product_that_drops_only_zeros_stays_exact() {
        let result = num("123456789.123456789")
            .mul(num("1000000000000"))
            .unwrap();

        assert!(result.exact);
        assert_eq!(result.text(), "123456789123456789000");
    }

    #[test]
    fn division_without_remainder_is_exact() {
        assert!(num("1").div(num("4")).unwrap().exact);
        assert!(!num("1").div(num("3")).unwrap().exact);
    }

    #[test]
    fn literal_with_too_many_digits_is_rounded_and_not_exact() {
        let result = num("0.123456789012345678901234567891");

        assert!(!result.exact);
    }

    #[test]
    fn literal_outside_the_range_is_too_large() {
        assert_eq!(Num::parse("1e40"), Err(CalcError::TooLarge));
        assert_eq!(
            Num::parse("123456789012345678901234567890123"),
            Err(CalcError::TooLarge)
        );
    }

    #[test]
    fn negative_zero_shows_as_zero() {
        let result = Num::exact(-Decimal::ZERO);

        assert_eq!(result.text(), "0");
    }

    #[test]
    fn money_has_two_decimal_places() {
        assert_eq!(money_text(Decimal::new(9610, 2)), "96.10");
        assert_eq!(money_text(Decimal::new(100, 0)), "100.00");
        assert_eq!(money_text(Decimal::ZERO), "0.00");
        assert_eq!(money_text(-Decimal::ZERO), "0.00");
        assert_eq!(money_text(Decimal::new(-12345, 3)), "-12.35");
    }

    #[test]
    fn money_below_one_has_four_significant_digits() {
        assert_eq!(money_text(Decimal::new(5, 1)), "0.5");
        assert_eq!(money_text(Decimal::new(61946354, 10)), "0.006195");
    }
}
