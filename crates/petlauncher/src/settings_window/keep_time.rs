//! R9: the time setting of the kept query as the window shows it: a number
//! with a unit, or Forever, or Disabled.

use petlauncher_config::KeepQuery;

/// The choices of the unit list, in the order of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepUnit {
    Seconds,
    Minutes,
    Forever,
    Disabled,
}

impl KeepUnit {
    pub const ALL: [Self; 4] = [Self::Seconds, Self::Minutes, Self::Forever, Self::Disabled];

    pub fn label(self) -> &'static str {
        match self {
            Self::Seconds => "For a time in seconds",
            Self::Minutes => "For a time in minutes",
            Self::Forever => "Forever",
            Self::Disabled => "No, clear it at each close",
        }
    }

    /// The position in the unit list.
    pub fn index(self) -> u32 {
        Self::ALL
            .iter()
            .position(|unit| *unit == self)
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(0)
    }

    pub fn from_index(index: u32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
            .unwrap_or(Self::Seconds)
    }

    /// The number has an effect only with a unit of time.
    pub fn has_number(self) -> bool {
        matches!(self, Self::Seconds | Self::Minutes)
    }
}

/// The number field and the unit list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeepTime {
    pub number: u64,
    pub unit: KeepUnit,
}

/// The number that the field shows when the setting has no number.
pub const INITIAL_SECONDS: u64 = 30;

impl From<KeepQuery> for KeepTime {
    /// Whole minutes show as minutes, so `"5m"` shows 5 Minutes.
    fn from(value: KeepQuery) -> Self {
        match value {
            KeepQuery::Seconds(seconds) if seconds > 0 && seconds % 60 == 0 => Self {
                number: seconds / 60,
                unit: KeepUnit::Minutes,
            },
            KeepQuery::Seconds(seconds) => Self {
                number: seconds,
                unit: KeepUnit::Seconds,
            },
            KeepQuery::Forever => Self {
                number: INITIAL_SECONDS,
                unit: KeepUnit::Forever,
            },
            KeepQuery::Disabled => Self {
                number: INITIAL_SECONDS,
                unit: KeepUnit::Disabled,
            },
        }
    }
}

impl KeepTime {
    /// The setting for `settings.toml`. A time of 0 is `Disabled`, because
    /// no query stays for 0 seconds.
    pub fn to_keep_query(self) -> KeepQuery {
        match self.unit {
            KeepUnit::Seconds | KeepUnit::Minutes if self.number == 0 => KeepQuery::Disabled,
            KeepUnit::Seconds => KeepQuery::Seconds(self.number),
            KeepUnit::Minutes => KeepQuery::Seconds(self.number.saturating_mul(60)),
            KeepUnit::Forever => KeepQuery::Forever,
            KeepUnit::Disabled => KeepQuery::Disabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_initial_setting_shows_30_seconds() {
        let time = KeepTime::from(KeepQuery::Seconds(30));

        assert_eq!(
            time,
            KeepTime {
                number: 30,
                unit: KeepUnit::Seconds
            }
        );
        assert_eq!(time.to_keep_query(), KeepQuery::Seconds(30));
    }

    #[test]
    fn whole_minutes_show_as_minutes() {
        let time = KeepTime::from(KeepQuery::Seconds(300));

        assert_eq!(time.unit, KeepUnit::Minutes);
        assert_eq!(time.number, 5);
        assert_eq!(time.to_keep_query(), KeepQuery::Seconds(300));
    }

    #[test]
    fn forever_and_disabled_keep_a_number_for_the_field() {
        for (setting, unit) in [
            (KeepQuery::Forever, KeepUnit::Forever),
            (KeepQuery::Disabled, KeepUnit::Disabled),
        ] {
            let time = KeepTime::from(setting);

            assert_eq!(time.unit, unit);
            assert_eq!(time.number, INITIAL_SECONDS);
            assert_eq!(time.to_keep_query(), setting);
        }
    }

    #[test]
    fn a_typed_number_with_its_unit_gives_the_setting() {
        let seconds = KeepTime {
            number: 45,
            unit: KeepUnit::Seconds,
        };
        let minutes = KeepTime {
            number: 2,
            unit: KeepUnit::Minutes,
        };

        assert_eq!(seconds.to_keep_query(), KeepQuery::Seconds(45));
        assert_eq!(minutes.to_keep_query(), KeepQuery::Seconds(120));
        assert_eq!(minutes.to_keep_query().to_string(), "2m");
    }

    #[test]
    fn a_time_of_zero_is_disabled() {
        let time = KeepTime {
            number: 0,
            unit: KeepUnit::Minutes,
        };

        assert_eq!(time.to_keep_query(), KeepQuery::Disabled);
        assert_eq!(
            KeepTime::from(KeepQuery::Seconds(0)).unit,
            KeepUnit::Seconds
        );
    }

    #[test]
    fn the_unit_list_has_a_fixed_order() {
        for unit in KeepUnit::ALL {
            assert_eq!(KeepUnit::from_index(unit.index()), unit);
        }
        assert_eq!(KeepUnit::from_index(99), KeepUnit::Seconds);
        assert!(KeepUnit::Minutes.has_number());
        assert!(!KeepUnit::Forever.has_number());
        assert_eq!(KeepUnit::Minutes.label(), "For a time in minutes");
    }
}
