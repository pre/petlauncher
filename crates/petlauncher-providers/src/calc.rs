//! The calculation row (R8, R15).

use std::sync::{Arc, RwLock};

use petlauncher_calc::{calculate, Rates};
use petlauncher_core::config::Settings;
use petlauncher_core::{Action, CancelToken, Icon, Provider, Row, RowKind};

use crate::sync::read;

/// The currency rates in effect. The rates thread replaces them.
pub(crate) type SharedRates = Arc<RwLock<Option<Rates>>>;

/// One calculation row when the query is a calculation. Enter copies the
/// number. A row with an error, for example a division by zero, copies
/// nothing.
pub(crate) struct CalcProvider {
    rates: SharedRates,
}

impl CalcProvider {
    pub(crate) fn new(rates: SharedRates) -> Self {
        Self { rates }
    }
}

impl Provider for CalcProvider {
    fn search(&self, text: &str, _settings: &Settings, _cancel: &CancelToken) -> Vec<Row> {
        let rates = read(&self.rates);
        let Some(calculation) = calculate(text, rates.as_ref()) else {
            return Vec::new();
        };
        let enter = calculation
            .copy_text()
            .map_or(Action::Nothing, |number| Action::Copy(number.to_owned()));
        let subtitle = match (&calculation.note, &enter) {
            (Some(note), _) => note.clone(),
            (None, Action::Copy(_)) => "Enter copies the result".to_owned(),
            (None, _) => String::new(),
        };
        vec![Row::new(RowKind::Calculation, calculation.text(), enter)
            .with_subtitle(subtitle)
            .with_icon(Icon::Name("accessories-calculator".to_owned()))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(provider: &CalcProvider, text: &str) -> Vec<Row> {
        provider.search(text, &Settings::default(), &CancelToken::new())
    }

    fn with_rates(rates: Option<Rates>) -> CalcProvider {
        CalcProvider::new(Arc::new(RwLock::new(rates)))
    }

    #[test]
    fn a_calculation_gives_one_row_that_copies_the_number() {
        let rows = rows(&with_rates(None), "42.3+68.7+14.1+0.5");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, RowKind::Calculation);
        assert_eq!(rows[0].title, "125.6");
        assert_eq!(rows[0].enter, Action::Copy("125.6".into()));
        assert_eq!(rows[0].subtitle, "Enter copies the result");
        assert_eq!(rows[0].local_file, None);
    }

    #[test]
    fn text_that_is_not_a_calculation_gives_no_row() {
        assert!(rows(&with_rates(None), "firefox").is_empty());
    }

    #[test]
    fn an_error_is_a_row_that_copies_nothing() {
        for (query, title) in [
            ("1/0", "Division by zero"),
            ("5 kg in MB", "Cannot convert mass to data size"),
            ("100 usd in eur", "No currency rates available"),
        ] {
            let rows = rows(&with_rates(None), query);

            assert_eq!(rows.len(), 1, "{query}");
            assert_eq!(rows[0].title, title, "{query}");
            assert_eq!(rows[0].enter, Action::Nothing, "{query}");
            assert_eq!(rows[0].kind, RowKind::Calculation, "{query}");
        }
    }

    #[test]
    fn a_currency_row_copies_the_number_and_shows_the_date_of_the_rates() {
        let rates = Rates::new("2026-11-25", &[("USD", "1.0512")]).unwrap();

        let rows = rows(&with_rates(Some(rates)), "100 usd in eur");

        assert_eq!(rows[0].title, "95.13 EUR");
        assert_eq!(rows[0].enter, Action::Copy("95.13".into()));
        assert_eq!(rows[0].subtitle, "ECB reference rates of 2026-11-25");
    }

    #[test]
    fn the_row_uses_the_rates_that_replace_the_old_rates() {
        let shared: SharedRates = Arc::new(RwLock::new(None));
        let provider = CalcProvider::new(Arc::clone(&shared));

        *shared.write().unwrap() = Some(Rates::new("2026-11-26", &[("USD", "2")]).unwrap());

        assert_eq!(rows(&provider, "10 usd in eur")[0].title, "5.00 EUR");
    }
}
