//! Find out when the owner saves `settings.toml` or `shortcuts.toml`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use notify::event::{AccessKind, AccessMode, ModifyKind};
use notify::{Event, EventKind, RecursiveMode, Watcher};

use crate::error::ConfigError;
use crate::paths::ConfigFile;

/// The time with no new event after which the watcher reports a change. An
/// editor makes several events for one save. The watcher reports the file
/// after the last of them, so that it does not read a half-written file.
pub const SETTLE_TIME: Duration = Duration::from_millis(100);

/// Watches the settings directory and calls a function when a file can
/// have new text.
///
/// The watcher watches the directory, not the files. Thus it sees a save
/// that writes the file in place, and a save that writes a temporary file
/// and renames it over the file, which gives a new inode. When a file is a
/// symbolic link, the watcher also watches the directory of the file that
/// the link points to at the start.
///
/// The watcher does not read the files and does not compare text. Give
/// each report to [`crate::ConfigStore::reload`], which reads the file and
/// gives [`crate::Reload::Unchanged`] after a write by petlauncher itself.
///
/// The function runs on a thread of the watcher. A GTK program sends the
/// value to its main loop, for example through an `async_channel` that a
/// future on the main context reads. The watch stops when the
/// `ConfigWatcher` is dropped.
pub struct ConfigWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl ConfigWatcher {
    /// Starts to watch `dir`, with [`SETTLE_TIME`]. Makes `dir` when it
    /// does not exist, because a directory that does not exist cannot be
    /// watched.
    pub fn start(
        dir: &Path,
        on_change: impl FnMut(ConfigFile) + Send + 'static,
    ) -> Result<Self, ConfigError> {
        Self::start_with_settle_time(dir, SETTLE_TIME, on_change)
    }

    pub fn start_with_settle_time(
        dir: &Path,
        settle_time: Duration,
        on_change: impl FnMut(ConfigFile) + Send + 'static,
    ) -> Result<Self, ConfigError> {
        let io_error = |error: std::io::Error| ConfigError::io(dir, &error);
        let notify_error = |error: notify::Error| ConfigError::Io {
            path: dir.to_owned(),
            message: error.to_string(),
        };
        fs::create_dir_all(dir).map_err(io_error)?;
        let targets = targets(dir);
        let (sender, events) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(sender).map_err(notify_error)?;
        for watched in directories_to_watch(dir, &targets) {
            watcher
                .watch(&watched, RecursiveMode::NonRecursive)
                .map_err(notify_error)?;
        }
        thread::Builder::new()
            .name("petlauncher-config-watch".to_owned())
            .spawn(move || report_changes(&events, &targets, settle_time, on_change))
            .map_err(io_error)?;
        Ok(Self { _watcher: watcher })
    }
}

/// The paths that name each file: the path in the directory, and the
/// path of the file that a symbolic link points to.
fn targets(dir: &Path) -> Vec<(PathBuf, ConfigFile)> {
    let mut targets = Vec::new();
    for file in ConfigFile::ALL {
        let path = file.path_in(dir);
        if let Ok(real) = fs::canonicalize(&path) {
            if real != path {
                targets.push((real, file));
            }
        }
        targets.push((path, file));
    }
    targets
}

fn directories_to_watch(dir: &Path, targets: &[(PathBuf, ConfigFile)]) -> Vec<PathBuf> {
    let real_dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_owned());
    let mut directories = vec![dir.to_owned()];
    for (path, _) in targets {
        if let Some(parent) = path.parent() {
            if parent != dir && parent != real_dir && !directories.iter().any(|d| d == parent) {
                directories.push(parent.to_owned());
            }
        }
    }
    directories
}

/// Collects the events and calls `on_change` for each file when the
/// settle time passed after its last event. Ends when the watcher is
/// dropped, because then the channel has no sender.
fn report_changes(
    events: &Receiver<notify::Result<Event>>,
    targets: &[(PathBuf, ConfigFile)],
    settle_time: Duration,
    mut on_change: impl FnMut(ConfigFile),
) {
    let mut due: Vec<(ConfigFile, Instant)> = Vec::new();
    loop {
        let received = match due.iter().map(|(_, time)| *time).min() {
            None => events.recv().map_err(|_| RecvTimeoutError::Disconnected),
            Some(time) => events.recv_timeout(time.saturating_duration_since(Instant::now())),
        };
        match received {
            Ok(event) => {
                for file in changed_files(&event, targets) {
                    due.retain(|(pending, _)| *pending != file);
                    due.push((file, Instant::now() + settle_time));
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let now = Instant::now();
        let (ready, waiting): (Vec<_>, Vec<_>) =
            due.into_iter().partition(|(_, time)| *time <= now);
        due = waiting;
        for (file, _) in ready {
            on_change(file);
        }
    }
}

/// The files that an event can have changed. An error of the watch, for
/// example a full event queue, can hide a change, so it gives both files.
fn changed_files(
    event: &notify::Result<Event>,
    targets: &[(PathBuf, ConfigFile)],
) -> Vec<ConfigFile> {
    let event = match event {
        Ok(event) if !event.need_rescan() => event,
        _ => return ConfigFile::ALL.to_vec(),
    };
    if !changes_content(&event.kind) {
        return Vec::new();
    }
    let mut files: Vec<ConfigFile> = targets
        .iter()
        .filter(|(path, _)| event.paths.contains(path))
        .map(|(_, file)| *file)
        .collect();
    files.dedup();
    files
}

/// `false` for the events that a read or a change of the permissions
/// gives. A read must not give a report, because the store reads the file
/// after each report.
fn changes_content(kind: &EventKind) -> bool {
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Any
        | EventKind::Create(_)
        | EventKind::Modify(_)
        | EventKind::Remove(_)
        | EventKind::Other => true,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    use super::*;

    const SETTLE: Duration = Duration::from_millis(50);
    const WAIT_FOR_EVENT: Duration = Duration::from_secs(5);
    const QUIET: Duration = Duration::from_millis(400);

    struct Watched {
        dir: tempfile::TempDir,
        changes: Receiver<ConfigFile>,
        watcher: ConfigWatcher,
    }

    fn watch(dir: tempfile::TempDir, watched: &Path) -> Watched {
        let (sender, changes) = mpsc::channel();
        let watcher = ConfigWatcher::start_with_settle_time(watched, SETTLE, move |file| {
            let _ = sender.send(file);
        })
        .expect("start the watcher");
        Watched {
            dir,
            changes,
            watcher,
        }
    }

    fn watch_new_dir() -> Watched {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().to_owned();
        watch(dir, &path)
    }

    impl Watched {
        fn path(&self, file: ConfigFile) -> PathBuf {
            file.path_in(self.dir.path())
        }

        fn next(&self) -> ConfigFile {
            self.changes
                .recv_timeout(WAIT_FOR_EVENT)
                .expect("a change report")
        }

        fn assert_quiet(&self) {
            assert_eq!(
                self.changes.recv_timeout(QUIET),
                Err(RecvTimeoutError::Timeout)
            );
        }
    }

    #[test]
    fn reports_a_file_that_an_editor_writes_in_place() {
        let watched = watch_new_dir();

        fs::write(watched.path(ConfigFile::Settings), "window_width = 800\n").expect("write");

        assert_eq!(watched.next(), ConfigFile::Settings);
        watched.assert_quiet();
    }

    #[test]
    fn reports_a_file_that_an_editor_replaces_with_a_rename() {
        let watched = watch_new_dir();
        let temporary = watched.dir.path().join(".shortcuts.toml.swp");

        fs::write(&temporary, "[[shortcut]]\n").expect("write");
        fs::rename(&temporary, watched.path(ConfigFile::Shortcuts)).expect("rename");

        assert_eq!(watched.next(), ConfigFile::Shortcuts);
        watched.assert_quiet();
    }

    #[test]
    fn reports_a_removed_file() {
        let watched = watch_new_dir();
        fs::write(watched.path(ConfigFile::Settings), "").expect("write");
        assert_eq!(watched.next(), ConfigFile::Settings);

        fs::remove_file(watched.path(ConfigFile::Settings)).expect("remove");

        assert_eq!(watched.next(), ConfigFile::Settings);
    }

    #[test]
    fn reports_many_quick_writes_as_one_change() {
        let watched = watch_new_dir();

        for width in 800..810 {
            fs::write(
                watched.path(ConfigFile::Settings),
                format!("window_width = {width}\n"),
            )
            .expect("write");
        }

        assert_eq!(watched.next(), ConfigFile::Settings);
        watched.assert_quiet();
    }

    #[test]
    fn ignores_other_files_and_reads() {
        let watched = watch_new_dir();
        fs::write(watched.path(ConfigFile::Settings), "").expect("write");
        assert_eq!(watched.next(), ConfigFile::Settings);

        fs::write(watched.dir.path().join("notes.txt"), "x").expect("write other");
        let _ = fs::read_to_string(watched.path(ConfigFile::Settings)).expect("read");

        watched.assert_quiet();
    }

    #[test]
    fn makes_a_missing_directory_and_watches_it() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let config = dir.path().join("petlauncher");
        let watched = watch(dir, &config);

        fs::write(ConfigFile::Shortcuts.path_in(&config), "").expect("write");

        assert_eq!(watched.next(), ConfigFile::Shortcuts);
    }

    #[test]
    fn reports_a_change_of_the_file_that_a_link_points_to() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let dotfiles = dir.path().join("dotfiles");
        let config = dir.path().join("config");
        fs::create_dir_all(&dotfiles).expect("dotfiles");
        fs::create_dir_all(&config).expect("config");
        let target = dotfiles.join("my-settings.toml");
        fs::write(&target, "").expect("write target");
        symlink(&target, ConfigFile::Settings.path_in(&config)).expect("symlink");
        let watched = watch(dir, &config);

        fs::write(&target, "window_width = 900\n").expect("write target");

        assert_eq!(watched.next(), ConfigFile::Settings);
    }

    #[test]
    fn stops_when_it_is_dropped() {
        let watched = watch_new_dir();
        let Watched {
            dir,
            changes,
            watcher,
        } = watched;

        drop(watcher);
        fs::write(ConfigFile::Settings.path_in(dir.path()), "").expect("write");

        assert_eq!(
            changes.recv_timeout(WAIT_FOR_EVENT),
            Err(RecvTimeoutError::Disconnected)
        );
    }

    #[test]
    fn gives_an_error_when_the_directory_cannot_be_made() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let file = dir.path().join("a-file");
        fs::write(&file, "").expect("write");

        let result = ConfigWatcher::start(&file.join("petlauncher"), |_| {});

        assert!(matches!(result, Err(ConfigError::Io { .. })));
    }
}
