//! The rates thread: it fetches the currency rates of the ECB (R15) in the
//! background, never on a thread that answers a query.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use petlauncher_calc::{parse_ecb, save_cache};
use petlauncher_core::Clock;
use petlauncher_time::jiff::tz::TimeZone;
use petlauncher_time::jiff::Timestamp;

use crate::background::{PendingGuard, Problems, Source};
use crate::calc::SharedRates;
use crate::launcher::{FetchRates, OnChange, Update};
use crate::sync::{read, write};

/// The time between two checks of the age of the rates.
pub(crate) const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

pub(crate) enum RatesCommand {
    /// Check now if the last check is `CHECK_INTERVAL` or more ago. The
    /// program sends this when the window opens, because the timer of the
    /// thread does not count the time in the suspend state.
    Check(PendingGuard),
}

/// Tells `Launcher::drop` if a fetch runs. Drop does not wait for a fetch
/// that runs, because a fetch can wait for the network until its timeout.
#[derive(Debug, Default)]
pub(crate) struct RatesFlags {
    pub stopped: AtomicBool,
    pub fetching: AtomicBool,
}

pub(crate) struct RatesJob {
    pub rates: SharedRates,
    pub cache_dir: PathBuf,
    pub fetch: FetchRates,
    pub clock: Arc<dyn Clock>,
    pub time_zone: TimeZone,
    pub on_change: OnChange,
    pub problems: Arc<Problems>,
    pub flags: Arc<RatesFlags>,
}

impl RatesJob {
    /// Checks at the start, then after each command and each
    /// `CHECK_INTERVAL`, until the sender of `commands` is dropped.
    pub(crate) fn run(self, start: PendingGuard, commands: Receiver<RatesCommand>) {
        let mut last_check = None;
        self.check(&mut last_check);
        drop(start);
        loop {
            match commands.recv_timeout(CHECK_INTERVAL) {
                Ok(RatesCommand::Check(_guard)) => self.check(&mut last_check),
                Err(RecvTimeoutError::Timeout) => self.check(&mut last_check),
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// Fetches when the last check is `CHECK_INTERVAL` or more ago and the
    /// rates in effect are older than the current date.
    fn check(&self, last_check: &mut Option<SystemTime>) {
        let now = self.clock.now();
        let due = last_check.is_none_or(|last| {
            now.duration_since(last)
                .map_or(true, |elapsed| elapsed >= CHECK_INTERVAL)
        });
        if !due {
            return;
        }
        *last_check = Some(now);
        if self.rates_are_old(now) {
            self.fetch();
        }
    }

    fn rates_are_old(&self, now: SystemTime) -> bool {
        let today = Timestamp::try_from(now).ok().map(|now| {
            now.to_zoned_datetime(self.time_zone.clone())
                .date()
                .to_string()
        });
        match read(&self.rates).as_ref() {
            None => true,
            Some(rates) => today.is_some_and(|today| rates.date() < today.as_str()),
        }
    }

    /// A failed fetch keeps the old rates, and is not an error for the
    /// owner: the calculation rows show the date of the rates.
    fn fetch(&self) {
        // With `SeqCst`, either this thread sees the stop, or the drop sees
        // the fetch and does not wait for it.
        self.flags.fetching.store(true, Ordering::SeqCst);
        if self.flags.stopped.load(Ordering::SeqCst) {
            self.flags.fetching.store(false, Ordering::SeqCst);
            return;
        }
        let fetched = (self.fetch)();
        self.flags.fetching.store(false, Ordering::SeqCst);
        let Ok(xml) = fetched else {
            return;
        };
        let Ok(new) = parse_ecb(&xml) else {
            return;
        };
        {
            let mut rates = write(&self.rates);
            if rates.as_ref().is_some_and(|old| old.date() > new.date()) {
                return;
            }
            *rates = Some(new);
        }
        let saved = save_cache(&self.cache_dir, &xml);
        self.problems.set(
            Source::RatesCache,
            saved
                .err()
                .map(|error| format!("The currency rates are not stored: {error}")),
        );
        if !self.flags.stopped.load(Ordering::SeqCst) {
            (self.on_change)(Update::Rates);
        }
    }
}
