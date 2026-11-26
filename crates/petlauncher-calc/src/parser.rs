//! Reads a query into a syntax tree, and decides if the query is a calculation.

use crate::functions::{Constant, Function};
use crate::lexer::{tokenize, Token};
use crate::units::{self, UnitRef};

/// A longer query is not a calculation. This also limits the work for a
/// pasted text.
const MAX_QUERY_CHARS: usize = 1000;
/// Brackets and signs deeper than this are not a calculation. This keeps the
/// recursion of the parser small.
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Expr {
    Number(String),
    Constant(Constant),
    Neg(Box<Expr>),
    Percent(Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(Function, Vec<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    /// `p% of x`. The left operand is a percent.
    Of,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Query {
    Value(Expr),
    Convert {
        value: Expr,
        from: UnitRef,
        to: UnitRef,
    },
}

impl Expr {
    /// `15%` or `-15%`. Such an operand of `+` and `-` is relative to the left operand.
    pub fn is_percent(&self) -> bool {
        match self {
            Expr::Percent(_) => true,
            Expr::Neg(inner) => matches!(**inner, Expr::Percent(_)),
            _ => false,
        }
    }

    /// False for a bare number or constant, also with a sign.
    fn has_operation(&self) -> bool {
        match self {
            Expr::Number(_) | Expr::Constant(_) => false,
            Expr::Neg(inner) => inner.has_operation(),
            _ => true,
        }
    }
}

/// Returns `None` when the query is not a calculation. See
/// `docs/calculator.md` for the rule.
pub(crate) fn parse(text: &str) -> Option<Query> {
    let text = text.trim();
    if text.chars().count() > MAX_QUERY_CHARS || looks_like_date(text) {
        return None;
    }
    let mut tokens = tokenize(text)?;

    // The owner is still typing: the row of `42+1` stays while `42+1+` is typed.
    if tokens.last().is_some_and(Token::is_binary_operator) {
        tokens.pop();
    }
    close_brackets(&mut tokens)?;

    let mut parser = Parser {
        tokens,
        position: 0,
        depth: 0,
    };
    let query = parser.query()?;
    let is_calculation = match &query {
        Query::Convert { .. } => true,
        Query::Value(expr) => expr.has_operation(),
    };
    is_calculation.then_some(query)
}

/// Three groups of digits with `-` or `/` between them, and a year of four
/// digits first or last: `2024-11-24`, `24/11/2024`, also `040-123-4567`.
fn looks_like_date(text: &str) -> bool {
    ['-', '/'].into_iter().any(|separator| {
        let groups: Vec<&str> = text.split(separator).collect();
        groups.len() == 3
            && groups
                .iter()
                .all(|group| !group.is_empty() && group.bytes().all(|b| b.is_ascii_digit()))
            && (groups[0].len() == 4 || groups[2].len() == 4)
    })
}

/// Adds the missing `)` at the end. Returns `None` for a `)` with no `(`.
fn close_brackets(tokens: &mut Vec<Token>) -> Option<()> {
    let mut open = 0usize;
    for token in tokens.iter() {
        match token {
            Token::Open => open += 1,
            Token::Close => open = open.checked_sub(1)?,
            _ => {}
        }
    }
    tokens.extend(std::iter::repeat_n(Token::Close, open));
    Some(())
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.position)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.position).cloned();
        self.position += 1;
        token
    }

    fn eat(&mut self, token: &Token) -> bool {
        let found = self.peek() == Some(token);
        if found {
            self.position += 1;
        }
        found
    }

    fn peek_keyword(&self, keyword: &str) -> bool {
        matches!(self.peek(), Some(Token::Name(name)) if name.eq_ignore_ascii_case(keyword))
    }

    fn at_end(&self) -> bool {
        self.position >= self.tokens.len()
    }

    /// `expr [unit (in | to) unit]`
    fn query(&mut self) -> Option<Query> {
        let value = self.expr()?;
        if self.at_end() {
            return Some(Query::Value(value));
        }
        let from = self.unit()?;
        if !(self.peek_keyword("in") || self.peek_keyword("to")) {
            return None;
        }
        self.position += 1;
        let to = self.unit()?;
        self.at_end().then_some(Query::Convert { value, from, to })
    }

    fn unit(&mut self) -> Option<UnitRef> {
        match self.next()? {
            Token::Name(name) => units::lookup(&name),
            _ => None,
        }
    }

    /// `term ((+ | -) term)*`
    fn expr(&mut self) -> Option<Expr> {
        let mut left = self.term()?;
        loop {
            let op = match self.peek() {
                Some(Token::Plus) => BinOp::Add,
                Some(Token::Minus) => BinOp::Sub,
                _ => return Some(left),
            };
            self.position += 1;
            let right = self.term()?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    /// `unary ((* | / | mod) unary)*`, and `percent of term`.
    fn term(&mut self) -> Option<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Token::Star) => BinOp::Mul,
                Some(Token::Slash) => BinOp::Div,
                _ if self.peek_keyword("mod") => BinOp::Mod,
                _ if self.peek_keyword("of") && left.is_percent() => BinOp::Of,
                _ => return Some(left),
            };
            self.position += 1;
            // `of` takes the rest of the term, so `50% of 50% of 240` is 60.
            let right = if op == BinOp::Of {
                self.term()?
            } else {
                self.unary()?
            };
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
    }

    /// `(- | +) unary | power`. Each recursion of the parser goes through here.
    fn unary(&mut self) -> Option<Expr> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return None;
        }
        let expr = if self.eat(&Token::Minus) {
            self.unary().map(|inner| Expr::Neg(Box::new(inner)))
        } else if self.eat(&Token::Plus) {
            self.unary()
        } else {
            self.power()
        };
        self.depth -= 1;
        expr
    }

    /// `postfix [^ unary]`. The exponent is a unary, so `2^-1` and `2^3^2` work.
    fn power(&mut self) -> Option<Expr> {
        let base = self.postfix()?;
        if self.eat(&Token::Caret) {
            let exponent = self.unary()?;
            return Some(Expr::Binary(BinOp::Pow, Box::new(base), Box::new(exponent)));
        }
        Some(base)
    }

    /// `primary [%]`
    fn postfix(&mut self) -> Option<Expr> {
        let primary = self.primary()?;
        if self.eat(&Token::Percent) {
            return Some(Expr::Percent(Box::new(primary)));
        }
        Some(primary)
    }

    /// `number | constant | function ( args ) | ( expr )`
    fn primary(&mut self) -> Option<Expr> {
        match self.next()? {
            Token::Number(literal) => Some(Expr::Number(literal)),
            Token::Open => {
                let inner = self.expr()?;
                self.eat(&Token::Close).then_some(inner)
            }
            Token::Name(name) if self.eat(&Token::Open) => {
                let function = Function::from_name(&name)?;
                let mut arguments = vec![self.expr()?];
                while self.eat(&Token::Comma) {
                    arguments.push(self.expr()?);
                }
                let fits = function.arity().contains(&arguments.len());
                (self.eat(&Token::Close) && fits).then_some(Expr::Call(function, arguments))
            }
            Token::Name(name) => Constant::from_name(&name).map(Expr::Constant),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_with_a_year_is_not_a_calculation() {
        assert!(looks_like_date("2024-11-24"));
        assert!(looks_like_date("24/11/2024"));
        assert!(!looks_like_date("1/2/3"));
        assert!(!looks_like_date("2024-11"));
        assert!(!looks_like_date("2024 - 11 - 24"));
    }

    #[test]
    fn percent_binds_tighter_than_power_and_sign() {
        let query = parse("-2^10%").unwrap();

        let expected = Expr::Neg(Box::new(Expr::Binary(
            BinOp::Pow,
            Box::new(Expr::Number("2".into())),
            Box::new(Expr::Percent(Box::new(Expr::Number("10".into())))),
        )));
        assert_eq!(query, Query::Value(expected));
    }

    #[test]
    fn inch_is_a_unit_before_the_keyword_in() {
        let query = parse("5 in in cm").unwrap();

        assert!(matches!(
            query,
            Query::Convert { from, to, .. } if from.symbol() == "in" && to.symbol() == "cm"
        ));
    }

    #[test]
    fn conversion_without_a_target_is_not_a_calculation() {
        assert_eq!(parse("5 GiB in"), None);
        assert_eq!(parse("5 GiB in MB extra"), None);
        assert_eq!(parse("5 GiB MB"), None);
    }
}
