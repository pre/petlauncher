//! The store and the watcher together, as the program uses them (R18).

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use petlauncher_config::{ConfigError, ConfigFile, ConfigStore, ConfigWatcher, Reload};

const WAIT_FOR_EVENT: Duration = Duration::from_secs(5);
const QUIET: Duration = Duration::from_millis(400);

struct Running {
    dir: tempfile::TempDir,
    store: ConfigStore,
    changes: Receiver<ConfigFile>,
    _watcher: ConfigWatcher,
}

impl Running {
    fn start(settings: &str) -> Self {
        let dir = tempfile::tempdir().expect("temporary directory");
        fs::write(ConfigFile::Settings.path_in(dir.path()), settings).expect("write settings");
        let store = ConfigStore::open(dir.path());
        let (sender, changes) = mpsc::channel();
        let watcher = ConfigWatcher::start_with_settle_time(
            dir.path(),
            Duration::from_millis(50),
            move |file| {
                let _ = sender.send(file);
            },
        )
        .expect("start the watcher");
        Self {
            dir,
            store,
            changes,
            _watcher: watcher,
        }
    }

    fn settings_path(&self) -> PathBuf {
        ConfigFile::Settings.path_in(self.dir.path())
    }

    /// Waits for the next report and gives it to the store, as the main
    /// loop of the program does.
    fn reload_next(&mut self) -> Reload {
        let file = self
            .changes
            .recv_timeout(WAIT_FOR_EVENT)
            .expect("a change report");
        self.store.reload(file)
    }

    fn assert_quiet(&self) {
        assert_eq!(
            self.changes.recv_timeout(QUIET),
            Err(RecvTimeoutError::Timeout)
        );
    }
}

#[test]
fn a_write_by_petlauncher_does_not_start_a_reload_loop() {
    let mut running = Running::start("window_width = 750\n");

    running
        .store
        .edit_settings(|editor| editor.set_window_width(900))
        .expect("edit");

    assert_eq!(running.reload_next(), Reload::Unchanged);
    running.assert_quiet();
    assert_eq!(running.store.settings().window_width, 900);
}

#[test]
fn a_save_that_renames_the_old_file_and_writes_a_new_file_goes_into_effect() {
    let mut running = Running::start("window_width = 750\n");
    let path = running.settings_path();
    let backup = running.dir.path().join("settings.toml~");

    fs::rename(&path, &backup).expect("rename to backup");
    fs::write(&path, "window_width = 1100\n").expect("write new file");
    fs::remove_file(&backup).expect("remove backup");

    assert_eq!(running.reload_next(), Reload::Changed);
    assert_eq!(running.store.settings().window_width, 1100);
    running.assert_quiet();
}

#[test]
fn a_save_with_an_error_keeps_the_old_settings_and_gives_the_error() {
    let mut running = Running::start("window_width = 750\n");

    fs::write(
        running.settings_path(),
        "window_width = 800\nkeep_query = \"soon\"\n",
    )
    .expect("write");

    assert_eq!(running.reload_next(), Reload::Failed);
    assert_eq!(running.store.settings().window_width, 750);
    let error = running.store.error(ConfigFile::Settings).expect("an error");
    assert_eq!(error.line(), Some(2));
    assert!(matches!(error, ConfigError::Toml { .. }));
    assert!(error.to_string().contains("soon"), "{error}");
}
