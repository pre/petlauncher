//! The seam between the windows and the rest of the program: the
//! `Launcher` of `petlauncher-providers` (the engine with all providers, the
//! settings, the kept query and the background work) and the GNOME desktop
//! of `petlauncher-gnome` (the actions, the tray icon action and the custom
//! shortcut). `docs/program.md` describes the rules.

use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use petlauncher_calc::RatesError;
use petlauncher_config::{ConfigFile, Settings};
use petlauncher_core::{Action, CloseReason, DesktopError, Key, Outcome, Results};
use petlauncher_gnome::GnomeDesktop;
use petlauncher_providers::{Autostart, Dirs, Hooks, Launcher, Update};

use crate::desktop;
use crate::log;
use crate::worker::SearchWorker;

/// When this environment variable is set, the program does not download
/// the currency rates and uses only the stored rates. The headless test
/// sets it, because the test has no network.
pub const OFFLINE_VAR: &str = "PETLAUNCHER_OFFLINE";

/// What the background work sends to the main loop.
pub struct Events {
    /// The results of each search that was not cancelled.
    pub results: async_channel::Receiver<Results>,
    /// The updates of the `Launcher`. After each one the program searches
    /// the query again.
    pub updates: async_channel::Receiver<Update>,
}

pub struct Backend {
    launcher: Arc<Launcher>,
    worker: RefCell<SearchWorker>,
    desktop: Result<GnomeDesktop, DesktopError>,
    config_dir: PathBuf,
    state_dir: PathBuf,
}

impl Backend {
    /// Starts the `Launcher` with the directories of the user and the hooks
    /// of the program: the GNOME rule for settings pages, the autostart
    /// file with this program and `--hidden` (R3), and no download when
    /// `PETLAUNCHER_OFFLINE` is set.
    pub fn start(desktop: Result<GnomeDesktop, DesktopError>) -> io::Result<(Self, Events)> {
        let dirs =
            Dirs::from_env().ok_or_else(|| io::Error::other("HOME is not an absolute path"))?;
        Self::start_with(dirs, desktop, program_hooks)
    }

    /// The same with given directories. `configure` changes the hooks, for
    /// example a test hook that never downloads.
    pub fn start_with(
        dirs: Dirs,
        desktop: Result<GnomeDesktop, DesktopError>,
        configure: impl FnOnce(&mut Hooks),
    ) -> io::Result<(Self, Events)> {
        let (update_sender, updates) = async_channel::unbounded();
        let mut hooks = Hooks::new(petlauncher_gnome::is_settings_page, move |update| {
            // The receiver stops only with the program.
            let _ = update_sender.send_blocking(update);
        });
        configure(&mut hooks);
        let config_dir = dirs.config.clone();
        let state_dir = dirs.state.clone();
        let launcher = Arc::new(Launcher::start(dirs, hooks)?);
        let searcher = Arc::clone(&launcher);
        let (result_sender, results) = async_channel::unbounded();
        let worker = SearchWorker::start(
            Box::new(move |query, cancel| searcher.search(query, cancel)),
            result_sender,
        );
        let backend = Self {
            launcher,
            worker: RefCell::new(worker),
            desktop,
            config_dir,
            state_dir,
        };
        Ok((backend, Events { results, updates }))
    }

    /// The GNOME desktop, or the error that its start gave.
    pub fn desktop(&self) -> Result<&GnomeDesktop, DesktopError> {
        self.desktop.as_ref().map_err(Clone::clone)
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    pub fn settings(&self) -> Arc<Settings> {
        self.launcher.settings()
    }

    /// Starts a search. The results come through `Events::results`.
    pub fn search(&self, query: &str) {
        self.worker.borrow_mut().search(query);
    }

    pub fn activate(&self, results: &Results, selected: Option<usize>, key: Key) -> Outcome {
        self.launcher.activate(results, selected, key)
    }

    /// Executes an action that needs the desktop. Call it while the
    /// launcher window is still visible.
    pub fn execute(&self, action: &Action) -> Result<(), DesktopError> {
        desktop::execute(self.desktop()?, action)
    }

    /// The program executed `outcome` for `query`. When `outcome.close` is
    /// true, this records the close of the window for R9, so do not also
    /// call `window_closed` for it.
    pub fn executed(&self, query: &str, outcome: &Outcome) {
        if let Err(error) = self.launcher.executed(query, outcome) {
            log::info(format_args!("the start counts are not saved: {error}"));
        }
    }

    /// Gives the kept query for the show of the window (R9), and starts the
    /// background checks of the `Launcher`.
    pub fn window_opened(&self) -> Option<String> {
        self.launcher.window_opened()
    }

    /// A close with no key on a row: Escape, a focus loss or the global
    /// key. `executed` records a close after a key.
    pub fn window_closed(&self, query: &str, reason: CloseReason) {
        self.launcher.window_closed(query, reason);
    }

    /// R18: the error of a settings file, whose old values stay in effect.
    pub fn config_notice(&self) -> Option<String> {
        ConfigFile::ALL.iter().find_map(|&file| {
            self.launcher.config_error(file).map(|error| {
                format!("{file} has an error, so its old values stay in effect: {error}")
            })
        })
    }

    /// The problems of the background work, for the log.
    pub fn problems(&self) -> Vec<String> {
        self.launcher.problems()
    }
}

fn program_hooks(hooks: &mut Hooks) {
    hooks.autostart = autostart_command().and_then(Autostart::from_env);
    if std::env::var_os(OFFLINE_VAR).is_some() {
        hooks.fetch_rates = Arc::new(offline_fetch);
    }
}

fn offline_fetch() -> Result<String, RatesError> {
    Err(RatesError::Http(format!(
        "no download, because {OFFLINE_VAR} is set"
    )))
}

/// The `Exec` value of the autostart file (R3): this program with
/// `--hidden`. `None` when the path cannot be written in a desktop entry.
fn autostart_command() -> Option<String> {
    let program = std::env::current_exe().ok()?;
    Some(format!("{} --hidden", exec_argument(program.to_str()?)?))
}

/// One argument for the `Exec` key of a desktop entry: quoted when it has a
/// reserved character of the Desktop Entry Specification, and `%` doubled.
/// `None` for a backslash or a line end, which also need the escapes of the
/// key file.
fn exec_argument(text: &str) -> Option<String> {
    const RESERVED: &[char] = &[
        ' ', '\t', '"', '\'', '>', '<', '~', '|', '&', ';', '$', '*', '?', '#', '(', ')', '`',
    ];
    if text.contains(['\\', '\n', '\r']) {
        return None;
    }
    let text = text.replace('%', "%%");
    if !text.contains(RESERVED) {
        return Some(text);
    }
    let mut quoted = String::from("\"");
    for character in text.chars() {
        if matches!(character, '"' | '`' | '$') {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    Some(quoted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use petlauncher_core::{Mode, TypedPath};
    use petlauncher_providers::Environment;
    use std::fs;
    use std::time::Duration;

    struct Fixture {
        _root: tempfile::TempDir,
        dirs: Dirs,
    }

    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir().unwrap();
            let home = root.path().join("home");
            fs::create_dir_all(home.join("projects/notes")).unwrap();
            let dirs = Dirs {
                config: home.join(".config/petlauncher"),
                cache: home.join(".cache/petlauncher"),
                state: home.join(".local/state/petlauncher"),
                applications: Environment {
                    data_dirs: vec![root.path().join("data")],
                    current_desktops: vec!["GNOME".to_owned()],
                    locale: None,
                    path: Vec::new(),
                },
                zoneinfo: root.path().join("zoneinfo"),
                city_list: root.path().join("Locations.xml"),
                home,
            };
            fs::create_dir_all(&dirs.config).unwrap();
            Self { _root: root, dirs }
        }

        fn start(&self) -> (Backend, Events) {
            let (backend, events) = Backend::start_with(
                self.dirs.clone(),
                Err(DesktopError("no desktop in the test".into())),
                |hooks| {
                    hooks.watch_config = false;
                    hooks.outside_ignore_files = false;
                    hooks.fetch_rates = Arc::new(offline_fetch);
                },
            )
            .unwrap();
            backend.launcher.wait_idle();
            (backend, events)
        }

        fn home(&self, relative: &str) -> PathBuf {
            self.dirs.home.join(relative)
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
        let fixture = Fixture::new();
        let (backend, events) = fixture.start();

        backend.search("~/projects/");

        let results = receive(&events.results);
        assert_eq!(results.mode, Mode::Path);
        assert_eq!(results.rows[0].title, "notes");
        assert_eq!(
            results.typed_path,
            Some(TypedPath::Directory(fixture.home("projects")))
        );
    }

    #[test]
    fn a_calculation_comes_from_the_providers_of_the_launcher() {
        let fixture = Fixture::new();
        let (backend, events) = fixture.start();

        backend.search("42.3+68.7+14.1+0.5");

        let results = receive(&events.results);
        assert_eq!(results.rows[0].title, "125.6");
    }

    #[test]
    fn r9_step_5_an_activation_that_closes_the_window_clears_the_query() {
        let fixture = Fixture::new();
        let (backend, events) = fixture.start();
        backend.search("~/projects");
        let results = receive(&events.results);
        let outcome = backend.activate(&results, results.selected, Key::Enter);

        backend.executed(&results.query, &outcome);
        let after_activation = backend.window_opened();
        backend.window_closed("~/projects", CloseReason::Escape);
        let after_escape = backend.window_opened();

        assert_eq!(outcome.action, Action::OpenFolder(fixture.home("projects")));
        assert_eq!(after_activation, None);
        assert_eq!(after_escape.as_deref(), Some("~/projects"));
    }

    #[test]
    fn an_action_without_a_desktop_gives_the_error_of_the_desktop() {
        let fixture = Fixture::new();
        let (backend, _events) = fixture.start();

        let result = backend.execute(&Action::OpenFolder(fixture.home("projects")));

        assert_eq!(result, Err(DesktopError("no desktop in the test".into())));
        assert!(backend.desktop().is_err());
    }

    #[test]
    fn a_settings_file_with_an_error_gives_a_notice_and_the_old_values() {
        let fixture = Fixture::new();
        fs::write(
            ConfigFile::Settings.path_in(&fixture.dirs.config),
            "window_width = \n",
        )
        .unwrap();

        let (backend, _events) = fixture.start();

        let notice = backend.config_notice().unwrap();
        assert!(notice.starts_with("settings.toml has an error"), "{notice}");
        assert_eq!(backend.settings().window_width, 750);
        assert_eq!(backend.config_dir(), fixture.dirs.config);
        assert_eq!(backend.state_dir(), fixture.dirs.state);
        assert!(backend.problems().iter().all(|problem| !problem.is_empty()));
    }

    #[test]
    fn the_offline_fetch_never_downloads() {
        let error = offline_fetch().unwrap_err();

        assert!(error.to_string().contains(OFFLINE_VAR), "{error}");
    }

    #[test]
    fn an_exec_argument_is_quoted_only_when_it_must_be() {
        assert_eq!(
            exec_argument("/home/a/.local/bin/petlauncher").as_deref(),
            Some("/home/a/.local/bin/petlauncher")
        );
        assert_eq!(
            exec_argument("/opt/my tools/petlauncher").as_deref(),
            Some("\"/opt/my tools/petlauncher\"")
        );
        assert_eq!(
            exec_argument("/opt/$x/\"q\"/100%").as_deref(),
            Some("\"/opt/\\$x/\\\"q\\\"/100%%\"")
        );
        assert_eq!(exec_argument("/opt/back\\slash"), None);
        assert_eq!(exec_argument("/opt/line\nend"), None);
    }

    #[test]
    fn the_autostart_command_is_this_program_with_hidden() {
        let command = autostart_command().unwrap();

        assert!(command.ends_with(" --hidden"), "{command}");
    }
}
