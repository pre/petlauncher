//! The loader thread, the count of background jobs, and the problems that
//! the background work found.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use petlauncher_time::Cities;

use crate::apps::AppsState;
use crate::launcher::{OnChange, Update};
use crate::sync::lock;

/// The number of background jobs that were asked for and did not end.
/// `Launcher::wait_idle` waits until it is zero.
#[derive(Debug, Default)]
pub(crate) struct Pending {
    count: Mutex<usize>,
    idle: Condvar,
}

impl Pending {
    /// Counts one job. The job ends when the guard is dropped, also when
    /// the thread of the job panics.
    pub(crate) fn begin(self: &Arc<Self>) -> PendingGuard {
        *lock(&self.count) += 1;
        PendingGuard(Arc::clone(self))
    }

    pub(crate) fn is_idle(&self) -> bool {
        *lock(&self.count) == 0
    }

    pub(crate) fn wait(&self) {
        let mut count = lock(&self.count);
        while *count > 0 {
            count = self
                .idle
                .wait(count)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}

#[derive(Debug)]
pub(crate) struct PendingGuard(Arc<Pending>);

impl Drop for PendingGuard {
    fn drop(&mut self) {
        let mut count = lock(&self.0.count);
        *count -= 1;
        if *count == 0 {
            self.0.idle.notify_all();
        }
    }
}

/// The part of petlauncher that has a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Source {
    Cities,
    Usage,
    RatesCache,
    Autostart,
    ConfigWatch,
}

/// The last problem of each source, as text for the owner.
#[derive(Debug, Default)]
pub(crate) struct Problems(Mutex<BTreeMap<Source, String>>);

impl Problems {
    pub(crate) fn set(&self, source: Source, problem: Option<String>) {
        let mut problems = lock(&self.0);
        match problem {
            Some(text) => problems.insert(source, text),
            None => problems.remove(&source),
        };
    }

    pub(crate) fn list(&self) -> Vec<String> {
        lock(&self.0).values().cloned().collect()
    }
}

pub(crate) enum LoaderJob {
    LoadCities(PendingGuard),
    CheckApplications(PendingGuard),
}

/// Loads the city list at the start, and reads the applications again when
/// their files changed.
pub(crate) struct Loader {
    pub cities: Arc<OnceLock<Cities>>,
    pub zoneinfo: PathBuf,
    pub city_list: PathBuf,
    pub apps: Arc<AppsState>,
    pub on_change: OnChange,
    pub problems: Arc<Problems>,
}

impl Loader {
    /// Does the jobs until the sender of `jobs` is dropped.
    pub(crate) fn run(self, jobs: Receiver<LoaderJob>) {
        for job in jobs {
            match job {
                LoaderJob::LoadCities(_guard) => self.load_cities(),
                LoaderJob::CheckApplications(_guard) => self.check_applications(),
            }
        }
    }

    fn load_cities(&self) {
        match Cities::load(&self.zoneinfo, &self.city_list) {
            Ok(cities) => {
                let _ = self.cities.set(cities);
                self.problems.set(Source::Cities, None);
                (self.on_change)(Update::Cities);
            }
            Err(error) => self.problems.set(
                Source::Cities,
                Some(format!("The time zone queries do not work: {error}")),
            ),
        }
    }

    fn check_applications(&self) {
        let list = self.apps.list();
        if list.has_changed() {
            self.apps.replace(list.reload());
            (self.on_change)(Update::Applications);
        }
    }
}
