//! The file index for the program: a snapshot that a background rescan
//! replaces. `docs/file-search.md` gives the rescan rule.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::index::{FileEntry, Index};
use crate::scan::scan_resolved;
use crate::scope::{Resolved, Scope, ScopeError};
use crate::store;

/// The file index of the program.
///
/// A search uses the current snapshot and does not wait for a scan. Share
/// the value between threads with an `Arc`. Not more than one scan runs at a
/// time. When the value is dropped, a running scan stops.
pub struct FileIndex {
    shared: Arc<Shared>,
}

struct Shared {
    file: PathBuf,
    snapshot: RwLock<Arc<Index>>,
    last_error: Mutex<Option<String>>,
    control: Mutex<Control>,
    on_update: Option<Box<dyn Fn() + Send + Sync>>,
}

struct Control {
    scope: Scope,
    resolved: Arc<Resolved>,
    running: Option<Running>,
}

struct Running {
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl FileIndex {
    /// Loads the stored index from `cache_dir` when it is valid for `scope`.
    /// If not, the index is empty until the first scan. Does not scan.
    pub fn open(scope: Scope, cache_dir: &Path) -> Result<Self, ScopeError> {
        Self::open_inner(scope, cache_dir, None)
    }

    /// Same as [`FileIndex::open`], and calls `on_update` on the scan thread
    /// each time a scan replaces the snapshot. A stopped scan does not call
    /// it. The program uses it to search again.
    pub fn open_with_listener(
        scope: Scope,
        cache_dir: &Path,
        on_update: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, ScopeError> {
        Self::open_inner(scope, cache_dir, Some(Box::new(on_update)))
    }

    fn open_inner(
        scope: Scope,
        cache_dir: &Path,
        on_update: Option<Box<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, ScopeError> {
        let resolved = Arc::new(scope.resolve()?);
        let file = cache_dir.join(store::FILE_NAME);
        let index = store::load(&file, &scope).unwrap_or_default();
        Ok(Self {
            shared: Arc::new(Shared {
                file,
                snapshot: RwLock::new(Arc::new(index)),
                last_error: Mutex::new(None),
                control: Mutex::new(Control {
                    scope,
                    resolved,
                    running: None,
                }),
                on_update,
            }),
        })
    }

    /// See [`Index::search`].
    pub fn search(&self, text: &str, limit: usize) -> Vec<FileEntry> {
        self.snapshot().search(text, limit)
    }

    /// See [`Index::search_files`].
    pub fn search_files(&self, text: &str, limit: usize) -> Vec<FileEntry> {
        self.snapshot().search_files(text, limit)
    }

    /// The current index. A rescan does not change it, but replaces it.
    pub fn snapshot(&self) -> Arc<Index> {
        Arc::clone(&self.shared.snapshot.read().unwrap())
    }

    /// Starts a scan in the background. When the scan ends, its index
    /// replaces the snapshot and the stored file. Returns false, and starts
    /// nothing, when a scan runs already.
    pub fn rescan(&self) -> bool {
        let mut control = self.shared.control.lock().unwrap();
        if control.running.as_ref().is_some_and(Running::is_active) {
            return false;
        }
        control.running = Some(self.start(&control));
        true
    }

    /// Changes the roots or the ignore entries. If the scope is different,
    /// stops a running scan and starts a new scan immediately (R21), and
    /// returns true. A bad scope gives an error and keeps the old scope.
    pub fn set_scope(&self, scope: Scope) -> Result<bool, ScopeError> {
        let resolved = Arc::new(scope.resolve()?);
        let mut control = self.shared.control.lock().unwrap();
        if control.scope == scope {
            return Ok(false);
        }
        if let Some(running) = control.running.take() {
            running.cancel.store(true, Ordering::Relaxed);
            let _ = running.thread.join();
        }
        control.scope = scope;
        control.resolved = resolved;
        control.running = Some(self.start(&control));
        Ok(true)
    }

    /// True when no scan made the index, or when the scan that made it
    /// started `age` or more ago.
    pub fn is_older_than(&self, age: Duration) -> bool {
        self.snapshot()
            .scanned_at()
            .is_none_or(|at| at.elapsed().map_or(true, |elapsed| elapsed >= age))
    }

    pub fn is_scanning(&self) -> bool {
        let control = self.shared.control.lock().unwrap();
        control.running.as_ref().is_some_and(Running::is_active)
    }

    /// Waits until the running scan ends.
    pub fn wait(&self) {
        let mut control = self.shared.control.lock().unwrap();
        if let Some(running) = control.running.take() {
            let _ = running.thread.join();
        }
    }

    /// The error of the last write of the stored file, if it failed.
    pub fn last_error(&self) -> Option<String> {
        self.shared.last_error.lock().unwrap().clone()
    }

    fn start(&self, control: &Control) -> Running {
        let cancel = Arc::new(AtomicBool::new(false));
        let shared = Arc::clone(&self.shared);
        let scope = control.scope.clone();
        let resolved = Arc::clone(&control.resolved);
        let stop = Arc::clone(&cancel);
        let thread = thread::spawn(move || shared.scan(&scope, &resolved, &stop));
        Running { cancel, thread }
    }
}

impl Drop for FileIndex {
    fn drop(&mut self) {
        if let Some(running) = self.shared.control.lock().unwrap().running.take() {
            running.cancel.store(true, Ordering::Relaxed);
        }
    }
}

impl Shared {
    fn scan(&self, scope: &Scope, resolved: &Resolved, cancel: &AtomicBool) {
        let Some(index) = scan_resolved(resolved, 0, cancel) else {
            return;
        };
        let index = Arc::new(index);
        *self.snapshot.write().unwrap() = Arc::clone(&index);
        if let Some(on_update) = &self.on_update {
            on_update();
        }
        let error = store::save(&self.file, scope, &index)
            .err()
            .map(|error| format!("cannot write {}: {error}", self.file.display()));
        *self.last_error.lock().unwrap() = error;
    }
}

impl Running {
    fn is_active(&self) -> bool {
        !self.thread.is_finished()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn empty_index(dir: &Path) -> FileIndex {
        let scope = Scope {
            roots: vec![],
            ignore: vec![],
            home: dir.to_path_buf(),
            outside_ignore_files: false,
        };
        FileIndex::open(scope, dir).unwrap()
    }

    /// Puts a scan in place that runs until `body` returns.
    fn fake_scan(index: &FileIndex, body: impl FnOnce(Arc<AtomicBool>) + Send + 'static) {
        let cancel = Arc::new(AtomicBool::new(false));
        let seen = Arc::clone(&cancel);
        let thread = thread::spawn(move || body(seen));
        index.shared.control.lock().unwrap().running = Some(Running { cancel, thread });
    }

    #[test]
    fn rescan_does_not_start_a_second_scan_while_one_runs() {
        let dir = tempfile::tempdir().unwrap();
        let index = empty_index(dir.path());
        let (release, released) = mpsc::channel::<()>();
        fake_scan(&index, move |_| {
            let _ = released.recv();
        });

        let second = index.rescan();
        let scanning = index.is_scanning();
        release.send(()).unwrap();
        index.wait();

        assert!(!second);
        assert!(scanning);
        assert!(!index.is_scanning());
        assert!(index.rescan());
        index.wait();
    }

    #[test]
    fn set_scope_stops_the_running_scan_before_it_starts_the_next() {
        let dir = tempfile::tempdir().unwrap();
        let index = empty_index(dir.path());
        let (report, reports) = mpsc::channel();
        fake_scan(&index, move |cancel| {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while !cancel.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
                thread::sleep(Duration::from_millis(1));
            }
            report.send(cancel.load(Ordering::Relaxed)).unwrap();
        });
        let mut scope = index.shared.control.lock().unwrap().scope.clone();
        scope.ignore.push("node_modules".into());

        let changed = index.set_scope(scope);

        assert_eq!(changed, Ok(true));
        assert_eq!(reports.try_recv(), Ok(true));
        index.wait();
    }

    #[test]
    fn drop_stops_the_running_scan() {
        let dir = tempfile::tempdir().unwrap();
        let index = empty_index(dir.path());
        let (report, reports) = mpsc::channel();
        fake_scan(&index, move |cancel| {
            let deadline = std::time::Instant::now() + Duration::from_secs(10);
            while !cancel.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
                thread::sleep(Duration::from_millis(1));
            }
            report.send(cancel.load(Ordering::Relaxed)).unwrap();
        });

        drop(index);

        assert_eq!(reports.recv_timeout(Duration::from_secs(20)), Ok(true));
    }
}
