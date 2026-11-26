//! R9: the query stays for a set time after the window closes.

use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use petlauncher_config::{KeepQuery, Settings};

/// Gives the time. The kept query uses the wall clock, so that the time in
/// the suspend state also counts.
pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

/// The clock of the system.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// A clock that moves only when `advance` is called. For tests.
#[derive(Debug)]
pub struct ManualClock(Mutex<SystemTime>);

impl ManualClock {
    pub fn new(start: SystemTime) -> Self {
        Self(Mutex::new(start))
    }

    pub fn advance(&self, duration: Duration) {
        let mut now = self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *now += duration;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self
            .0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Why the window closed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// A different window got the focus.
    FocusLoss,
    Escape,
    /// The global key (`petlauncher toggle`) hid the window.
    GlobalKey,
    /// A key on a row closed the window. Use `Outcome::keep_query`.
    Activated {
        keep_query: bool,
    },
}

impl CloseReason {
    fn keeps_query(self) -> bool {
        match self {
            Self::FocusLoss | Self::Escape | Self::GlobalKey => true,
            Self::Activated { keep_query } => keep_query,
        }
    }
}

/// The state of the kept query. Call `window_closed` when the window closes
/// and `window_opened` when it opens.
pub struct KeptQuery {
    clock: Arc<dyn Clock>,
    kept: Option<(String, SystemTime)>,
}

impl KeptQuery {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self { clock, kept: None }
    }

    pub fn window_closed(&mut self, query: &str, reason: CloseReason, settings: &Settings) {
        let keep = reason.keeps_query()
            && settings.run_in_background
            && settings.keep_query != KeepQuery::Disabled
            && !query.trim().is_empty();
        self.kept = keep.then(|| (query.to_owned(), self.clock.now()));
    }

    /// Gives the query to show, or `None` for an empty query line. The user
    /// interface selects all the text of the query and searches it again.
    /// The settings at the time of the open decide.
    pub fn window_opened(&mut self, settings: &Settings) -> Option<String> {
        let (query, closed_at) = self.kept.take()?;
        if !settings.run_in_background {
            return None;
        }
        let elapsed = self
            .clock
            .now()
            .duration_since(closed_at)
            .unwrap_or_default();
        let still_kept = match settings.keep_query {
            KeepQuery::Disabled => false,
            KeepQuery::Forever => true,
            KeepQuery::Seconds(seconds) => elapsed < Duration::from_secs(seconds),
        };
        still_kept.then_some(query)
    }
}

/// The warning that the settings window shows at the time setting of R9, or
/// `None` when the setting has effect.
pub fn keep_query_warning(settings: &Settings) -> Option<&'static str> {
    (!settings.run_in_background).then_some(
        "The query is not kept, because \"Run in background\" is off. \
         petlauncher stops when its window closes.",
    )
}
