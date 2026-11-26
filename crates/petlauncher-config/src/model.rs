use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The contents of `settings.toml`. A key that the file does not have gets
/// its initial value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub close_on_focus_loss: bool,
    pub run_in_background: bool,
    pub show_tray_icon: bool,
    pub include_applications: bool,
    /// The global key in the GNOME accelerator syntax, for example
    /// `<Super>space`.
    pub hotkey: String,
    pub window_width: u32,
    pub window_position: WindowPosition,
    /// The place of `WindowPosition::Custom`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_position: Option<CustomPosition>,
    pub keep_query: KeepQuery,
    pub file_search: FileSearch,
    pub dictionary: Dictionary,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            close_on_focus_loss: true,
            run_in_background: true,
            show_tray_icon: true,
            include_applications: true,
            hotkey: "<Super>space".to_owned(),
            window_width: 750,
            window_position: WindowPosition::Center,
            custom_position: None,
            keep_query: KeepQuery::Seconds(30),
            file_search: FileSearch::default(),
            dictionary: Dictionary::default(),
        }
    }
}

/// The place of the launcher window when it opens with no kept query, on the
/// monitor with the pointer (ADR 0009). The GNOME Shell extension puts the
/// window there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowPosition {
    /// The query line at the vertical center of the work area.
    Center,
    UpperThird,
    /// Below the top bar.
    Top,
    /// The place in `custom_position`.
    Custom,
}

impl WindowPosition {
    /// The text in `settings.toml`, for example `upper-third`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Center => "center",
            Self::UpperThird => "upper-third",
            Self::Top => "top",
            Self::Custom => "custom",
        }
    }
}

/// A place that the owner pinned, as fractions of the work area of the
/// monitor, so that it fits each monitor.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CustomPosition {
    /// The horizontal center of the window: 0 is the left edge of the work
    /// area, 1 the right edge.
    pub x: f64,
    /// The top of the query line: 0 is the top of the work area, 1 the
    /// bottom.
    pub y: f64,
}

/// How long the query stays after the window closes.
///
/// The TOML text is `"disabled"`, `"forever"`, a number of seconds such as
/// `"30s"`, or a number of minutes such as `"5m"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum KeepQuery {
    Disabled,
    Seconds(u64),
    Forever,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseKeepQueryError(pub String);

impl fmt::Display for ParseKeepQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" is not \"disabled\", \"forever\", seconds such as \"30s\" or minutes such as \"5m\"",
            self.0
        )
    }
}

impl std::error::Error for ParseKeepQueryError {}

impl FromStr for KeepQuery {
    type Err = ParseKeepQueryError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || ParseKeepQueryError(text.to_owned());
        let text = text.trim();
        match text {
            "disabled" => return Ok(Self::Disabled),
            "forever" => return Ok(Self::Forever),
            _ => {}
        }
        let (number, seconds_per_unit) = if let Some(number) = text.strip_suffix('s') {
            (number, 1)
        } else if let Some(number) = text.strip_suffix('m') {
            (number, 60)
        } else {
            return Err(error());
        };
        let number: u64 = number.trim().parse().map_err(|_| error())?;
        number
            .checked_mul(seconds_per_unit)
            .map(Self::Seconds)
            .ok_or_else(error)
    }
}

impl TryFrom<String> for KeepQuery {
    type Error = ParseKeepQueryError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl fmt::Display for KeepQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("disabled"),
            Self::Forever => f.write_str("forever"),
            Self::Seconds(seconds) if seconds % 60 == 0 && *seconds > 0 => {
                write!(f, "{}m", seconds / 60)
            }
            Self::Seconds(seconds) => write!(f, "{seconds}s"),
        }
    }
}

impl From<KeepQuery> for String {
    fn from(value: KeepQuery) -> Self {
        value.to_string()
    }
}

/// The `[file_search]` table: the file search with a keyword.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FileSearch {
    pub keyword: String,
    /// The directories that the index contains. `~` is the home directory.
    pub directories: Vec<String>,
    /// Each entry is a path such as `~/tmp`, a directory name such as
    /// `node_modules`, or a pattern for a directory name such as `*-wt`.
    pub ignore: Vec<String>,
}

impl Default for FileSearch {
    fn default() -> Self {
        Self {
            keyword: "f".to_owned(),
            directories: vec!["~".to_owned()],
            ignore: [
                "node_modules",
                "target",
                "__pycache__",
                "~/go/pkg/mod",
                "~/tmp",
                "git-worktrees",
                "*-wt",
            ]
            .map(str::to_owned)
            .to_vec(),
        }
    }
}

/// The `[dictionary]` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Dictionary {
    pub keyword: String,
    /// The directory with the WordNet files `index.noun`, `data.noun` and
    /// so on.
    pub wordnet_path: String,
}

impl Default for Dictionary {
    fn default() -> Self {
        Self {
            keyword: "d".to_owned(),
            wordnet_path: "/usr/share/wordnet".to_owned(),
        }
    }
}

/// One `[[shortcut]]` block of `shortcuts.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcut {
    pub name: String,
    pub keyword: String,
    /// A URL or a script. `%s` becomes the argument. A script also gets the
    /// argument as `$1`.
    pub cmd: String,
    /// The target when the query has no argument. Empty: the shortcut obeys
    /// `run_without_argument`.
    pub cmd_without_argument: String,
    pub icon: String,
    pub is_default_search: bool,
    pub run_without_argument: bool,
    /// The browser for a URL. `None`: the default browser.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<Browser>,
}

/// The browser of a shortcut, as `browser = { app = "..." }` or
/// `browser = { command = "..." }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Browser {
    /// The desktop entry ID of an installed browser, for example
    /// `microsoft-edge.desktop`.
    App(String),
    /// A free command. The URL is added as the last argument.
    Command(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_seconds_and_minutes_of_keep_query() {
        assert_eq!("30s".parse(), Ok(KeepQuery::Seconds(30)));
        assert_eq!("5m".parse(), Ok(KeepQuery::Seconds(300)));
        assert_eq!("forever".parse(), Ok(KeepQuery::Forever));
        assert_eq!("disabled".parse(), Ok(KeepQuery::Disabled));
    }

    #[test]
    fn rejects_keep_query_without_a_unit() {
        assert!("30".parse::<KeepQuery>().is_err());
        assert!("soon".parse::<KeepQuery>().is_err());
    }

    #[test]
    fn writes_keep_query_as_the_text_that_it_parses() {
        for value in [
            KeepQuery::Disabled,
            KeepQuery::Forever,
            KeepQuery::Seconds(30),
            KeepQuery::Seconds(300),
            KeepQuery::Seconds(0),
        ] {
            assert_eq!(value.to_string().parse(), Ok(value));
        }
    }
}
