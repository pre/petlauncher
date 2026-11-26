//! The time rows (R20).

use std::sync::{Arc, OnceLock};

use petlauncher_core::config::Settings;
use petlauncher_core::{Action, CancelToken, Clock, Icon, Provider, Row, RowKind};
use petlauncher_time::jiff::tz::TimeZone;
use petlauncher_time::jiff::Timestamp;
use petlauncher_time::Cities;

/// The rows of `Cities::answer`. The loader thread sets the city list after
/// the start. Until then, the provider gives no rows.
pub(crate) struct TimeProvider {
    cities: Arc<OnceLock<Cities>>,
    clock: Arc<dyn Clock>,
    time_zone: TimeZone,
}

impl TimeProvider {
    pub(crate) fn new(
        cities: Arc<OnceLock<Cities>>,
        clock: Arc<dyn Clock>,
        time_zone: TimeZone,
    ) -> Self {
        Self {
            cities,
            clock,
            time_zone,
        }
    }
}

impl Provider for TimeProvider {
    fn search(&self, text: &str, _settings: &Settings, _cancel: &CancelToken) -> Vec<Row> {
        let Some(cities) = self.cities.get() else {
            return Vec::new();
        };
        let Ok(now) = Timestamp::try_from(self.clock.now()) else {
            return Vec::new();
        };
        let now = now.to_zoned_datetime(self.time_zone.clone());
        cities
            .answer(text, &now)
            .into_iter()
            .map(|answer| {
                let enter = answer.copy.map_or(Action::Nothing, Action::Copy);
                Row::new(RowKind::Time, answer.title, enter)
                    .with_subtitle(answer.subtitle)
                    .with_icon(Icon::Name("preferences-system-time".to_owned()))
            })
            .collect()
    }
}
