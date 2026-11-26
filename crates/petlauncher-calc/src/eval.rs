//! Calculates the value of a parsed query.

use crate::number::{money_text, Num};
use crate::parser::{BinOp, Expr, Query};
use crate::units::{self, UnitRef};
use crate::{Answer, CalcError, Calculation, Rates};

pub(crate) fn evaluate(query: &Query, rates: Option<&Rates>) -> Calculation {
    match query {
        Query::Value(expr) => Calculation {
            result: value(expr).map(|number| Answer {
                number: number.text(),
                unit: None,
            }),
            note: None,
        },
        Query::Convert { value, from, to } => convert(value, *from, *to, rates),
    }
}

fn convert(expr: &Expr, from: UnitRef, to: UnitRef, rates: Option<&Rates>) -> Calculation {
    let number = value(expr).and_then(|amount| match (from, to) {
        (UnitRef::Unit(from), UnitRef::Unit(to)) => units::convert(amount, from, to).map(Num::text),
        (UnitRef::Currency(from), UnitRef::Currency(to)) => {
            currency(amount, from, to, rates.ok_or(CalcError::NoRates)?)
        }
        _ => Err(CalcError::IncompatibleUnits {
            from: from.dimension(),
            to: to.dimension(),
        }),
    });
    let note = match (from, to, rates) {
        (UnitRef::Currency(_), UnitRef::Currency(_), Some(rates)) => {
            Some(format!("ECB reference rates of {}", rates.date()))
        }
        _ => None,
    };
    Calculation {
        result: number.map(|number| Answer {
            number,
            unit: Some(to.symbol().to_string()),
        }),
        note,
    }
}

/// Converts through EUR, because the ECB gives each rate against EUR.
fn currency(amount: Num, from: &str, to: &str, rates: &Rates) -> Result<String, CalcError> {
    let rate = |code: &str| {
        rates
            .per_eur(code)
            .map(Num::exact)
            .ok_or_else(|| CalcError::MissingRate(code.to_string()))
    };
    let converted = amount.mul(rate(to)?)?.div(rate(from)?)?;
    Ok(money_text(converted.value))
}

fn value(expr: &Expr) -> Result<Num, CalcError> {
    match expr {
        Expr::Number(literal) => Num::parse(literal),
        Expr::Constant(constant) => Ok(constant.value()),
        Expr::Neg(inner) => Ok(value(inner)?.neg()),
        Expr::Percent(inner) => value(inner)?.div(Num::from_int(100)),
        Expr::Call(function, arguments) => {
            let arguments = arguments.iter().map(value).collect::<Result<Vec<_>, _>>()?;
            function.apply(&arguments)
        }
        Expr::Binary(op, left, right) => {
            let (a, b) = (value(left)?, value(right)?);
            match op {
                BinOp::Add if right.is_percent() => a.mul(Num::from_int(1).add(b)?),
                BinOp::Sub if right.is_percent() => a.mul(Num::from_int(1).sub(b)?),
                BinOp::Add => a.add(b),
                BinOp::Sub => a.sub(b),
                BinOp::Mul | BinOp::Of => a.mul(b),
                BinOp::Div => a.div(b),
                BinOp::Mod => a.rem(b),
                BinOp::Pow => a.pow(b),
            }
        }
    }
}
