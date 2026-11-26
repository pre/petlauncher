//! The seam between the user interface and the rest of the program.
//!
//! The windows call only the methods of `Backend`: `search`, `activate`,
//! `window_opened`, `window_closed`, `execute`, and `settings` with
//! `reload`. Now `Backend` joins `petlauncher-core` and `petlauncher-config`
//! directly, and the engine has only its built-in rows: the path mode, the
//! shortcuts and the system commands. The next task replaces the body of
//! this file with the `Launcher` of `petlauncher-providers` and the
//! `Desktop` of `petlauncher-gnome`. `docs/program.md` lists the changes.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::{Arc, PoisonError, RwLock};

use petlauncher_config::{ConfigFile, ConfigStore, Reload, Settings, Shortcut};
use petlauncher_core::{
    Action, CancelToken, CloseReason, DesktopError, Engine, KeptQuery, Key, Outcome, Results,
    SystemClock,
};

use crate::desktop;
use crate::log;
use crate::worker::SearchWorker;

/// The values of the two settings files that a search uses.
struct SearchConfig {
    settings: Settings,
    shortcuts: Vec<Shortcut>,
}

impl SearchConfig {
    fn of(store: &ConfigStore) -> Arc<Self> {
        Arc::new(Self {
            settings: store.settings().clone(),
            shortcuts: store.shortcuts().to_vec(),
        })
    }
}

pub struct Backend {
    store: RefCell<ConfigStore>,
    /// The worker reads it for each search, the main thread replaces it
    /// after a change of a settings file.
    search_config: Arc<RwLock<Arc<SearchConfig>>>,
    kept: RefCell<KeptQuery>,
    worker: RefCell<SearchWorker>,
}

impl Backend {
    /// Reads the settings in `config_dir` and starts the search thread. The
    /// results of each search come through the receiver.
    pub fn new(config_dir: PathBuf, home: PathBuf) -> (Self, async_channel::Receiver<Results>) {
        let store = ConfigStore::open(config_dir);
        for file in ConfigFile::ALL {
            if let Some(error) = store.error(file) {
                log::info(format_args!("{error}"));
            }
        }
        let search_config = Arc::new(RwLock::new(SearchConfig::of(&store)));
        let engine = Engine::new(home);
        let shared = Arc::clone(&search_config);
        let search = move |query: &str, cancel: &CancelToken| {
            let config = Arc::clone(&shared.read().unwrap_or_else(PoisonError::into_inner));
            engine.search(query, &config.settings, &config.shortcuts, cancel)
        };
        let (sender, receiver) = async_channel::unbounded();
        let backend = Self {
            store: RefCell::new(store),
            search_config,
            kept: RefCell::new(KeptQuery::new(Arc::new(SystemClock))),
            worker: RefCell::new(SearchWorker::start(Box::new(search), sender)),
        };
        (backend, receiver)
    }

    pub fn config_dir(&self) -> PathBuf {
        self.store.borrow().dir().to_owned()
    }

    pub fn settings(&self) -> Settings {
        self.store.borrow().settings().clone()
    }

    /// Reads a settings file again after the watcher reported a change.
    pub fn reload(&self, file: ConfigFile) -> Reload {
        let mut store = self.store.borrow_mut();
        let reload = store.reload(file);
        match reload {
            Reload::Changed => {
                *self
                    .search_config
                    .write()
                    .unwrap_or_else(PoisonError::into_inner) = SearchConfig::of(&store);
                log::info(format_args!("read {}", file.path_in(store.dir()).display()));
            }
            Reload::Failed => {
                if let Some(error) = store.error(file) {
                    log::info(format_args!("{error}"));
                }
            }
            Reload::Unchanged => {}
        }
        reload
    }

    /// Starts a search. The results come through the receiver of `new`.
    pub fn search(&self, query: &str) {
        self.worker.borrow_mut().search(query);
    }

    pub fn activate(&self, results: &Results, selected: Option<usize>, key: Key) -> Outcome {
        petlauncher_core::activate(results, selected, key)
    }

    /// Gives the kept query for the show of the window (R9).
    pub fn window_opened(&self) -> Option<String> {
        let settings = self.settings();
        self.kept.borrow_mut().window_opened(&settings)
    }

    pub fn window_closed(&self, query: &str, reason: CloseReason) {
        let settings = self.settings();
        self.kept
            .borrow_mut()
            .window_closed(query, reason, &settings);
    }

    /// Executes an action that needs the desktop.
    pub fn execute(&self, action: &Action) -> Result<(), DesktopError> {
        desktop::execute(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petlauncher_core::{Mode, TypedPath};
    use std::fs;
    use std::time::Duration;

    struct Fixture {
        _root: tempfile::TempDir,
        home: PathBuf,
        config: PathBuf,
    }

    fn fixture() -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let config = root.path().join("config");
        fs::create_dir_all(home.join("projects/notes")).unwrap();
        fs::create_dir_all(&config).unwrap();
        Fixture {
            home,
            config,
            _root: root,
        }
    }

    fn receive(receiver: &async_channel::Receiver<Results>) -> Results {
        let (sender, done) = std::sync::mpsc::channel();
        let receiver = receiver.clone();
        std::thread::spawn(move || {
            let _ = sender.send(receiver.recv_blocking());
        });
        done.recv_timeout(Duration::from_secs(5))
            .expect("no results in 5 s")
            .unwrap()
    }

    #[test]
    fn searches_a_typed_directory_on_the_worker() {
        let fixture = fixture();
        let (backend, receiver) = Backend::new(fixture.config.clone(), fixture.home.clone());

        backend.search("~/projects/");

        let results = receive(&receiver);
        assert_eq!(results.mode, Mode::Path);
        assert_eq!(results.rows[0].title, "notes");
        assert_eq!(
            results.typed_path,
            Some(TypedPath::Directory(fixture.home.join("projects")))
        );
    }

    #[test]
    fn enter_with_no_selected_row_opens_the_typed_directory() {
        let fixture = fixture();
        let (backend, receiver) = Backend::new(fixture.config.clone(), fixture.home.clone());
        backend.search("~/projects");
        let results = receive(&receiver);

        let outcome = backend.activate(&results, results.selected, Key::Enter);

        assert_eq!(
            outcome.action,
            Action::OpenFolder(fixture.home.join("projects"))
        );
        assert!(outcome.close);
    }

    #[test]
    fn uses_the_shortcuts_of_the_settings_directory_after_a_reload() {
        let fixture = fixture();
        let (backend, receiver) = Backend::new(fixture.config.clone(), fixture.home.clone());
        fs::write(
            ConfigFile::Shortcuts.path_in(&fixture.config),
            "[[shortcut]]\nname = \"Search\"\nkeyword = \"g\"\ncmd = \"https://example.com/?q=%s\"\n",
        )
        .unwrap();

        let reload = backend.reload(ConfigFile::Shortcuts);
        backend.search("g rust");

        let results = receive(&receiver);
        assert_eq!(reload, Reload::Changed);
        assert_eq!(results.mode, Mode::Shortcut);
        assert_eq!(
            results.rows[0].enter,
            Action::OpenUrl {
                url: "https://example.com/?q=rust".into(),
                browser: None
            }
        );
    }

    #[test]
    fn keeps_the_old_settings_when_the_file_has_an_error() {
        let fixture = fixture();
        fs::write(
            ConfigFile::Settings.path_in(&fixture.config),
            "window_width = 900\n",
        )
        .unwrap();
        let (backend, _receiver) = Backend::new(fixture.config.clone(), fixture.home.clone());
        fs::write(
            ConfigFile::Settings.path_in(&fixture.config),
            "window_width = \n",
        )
        .unwrap();

        let reload = backend.reload(ConfigFile::Settings);

        assert_eq!(reload, Reload::Failed);
        assert_eq!(backend.settings().window_width, 900);
        assert_eq!(backend.config_dir(), fixture.config);
    }

    #[test]
    fn keeps_the_query_after_escape_and_not_after_an_activation() {
        let fixture = fixture();
        let (backend, _receiver) = Backend::new(fixture.config.clone(), fixture.home.clone());

        backend.window_closed("~/projects/", CloseReason::Escape);
        let after_escape = backend.window_opened();
        backend.window_closed("~/projects/", CloseReason::Activated { keep_query: false });
        let after_activation = backend.window_opened();

        assert_eq!(after_escape.as_deref(), Some("~/projects/"));
        assert_eq!(after_activation, None);
    }

    #[test]
    fn executes_a_desktop_action_at_the_seam() {
        let fixture = fixture();
        let (backend, _receiver) = Backend::new(fixture.config.clone(), fixture.home.clone());

        let result = backend.execute(&Action::OpenFolder(fixture.home.clone()));

        assert_eq!(result, Ok(()));
    }
}
