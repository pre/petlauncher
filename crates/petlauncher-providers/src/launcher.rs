//! The `Launcher`: the one object that the program holds. See
//! `docs/launcher.md` for the surface, the threads and the background rules.

use std::io;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime};

use petlauncher_apps::{Applications, DesktopEntry, Environment, SettingsPageRule, UsageStore};
use petlauncher_calc::{load_cache, RatesError};
use petlauncher_config::{
    Browser, ConfigError, ConfigFile, ConfigStore, ConfigWatcher, EditError, FileSearch, Problem,
    Reload, Settings, SettingsEditor, Shortcut, ShortcutsEditor,
};
use petlauncher_core::{
    Action, CancelToken, Clock, CloseReason, Engine, KeptQuery, Key, Outcome, Results, Slot,
    SystemClock,
};
use petlauncher_dict::{Entry, Error as DictionaryError};
use petlauncher_index::{FileIndex, ScopeError};
use petlauncher_time::jiff::tz::TimeZone;
use petlauncher_time::GWEATHER_LOCATIONS;

use crate::apps::{ApplicationsProvider, AppsState, SettingsPagesProvider};
use crate::autostart::Autostart;
use crate::background::{Loader, LoaderJob, Pending, Problems, Source};
use crate::calc::{CalcProvider, SharedRates};
use crate::dictionary::{DictionaryProvider, DictionaryState};
use crate::files::{self, FileSearchProvider};
use crate::rates::{RatesCommand, RatesFlags, RatesJob};
use crate::sync::{lock, read, write};
use crate::time::TimeProvider;

/// The function that `Hooks::on_change` holds.
pub type OnChange = Arc<dyn Fn(Update) + Send + Sync>;

/// The function that downloads the ECB rates file and gives its text.
pub type FetchRates = Arc<dyn Fn() -> Result<String, RatesError> + Send + Sync>;

/// When the window opens and the last scan of the file index started this
/// time or more ago, the index scans again (R14).
pub(crate) const RESCAN_AGE: Duration = Duration::from_secs(60);

/// The timeout of the download of the ECB rates.
pub(crate) const FETCH_TIMEOUT: Duration = Duration::from_secs(10);

/// What changed in the background. The program searches the query again,
/// and after `Config` it also shows the new settings and errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Update {
    /// New currency rates.
    Rates,
    /// The city list for the time queries is loaded.
    Cities,
    /// A scan of the file index ended.
    Files,
    /// The list of applications was read again.
    Applications,
    /// The watcher read a settings file again. The new values are in
    /// effect, or the file has an error and the old values stay.
    Config(ConfigFile),
}

/// The directories and the environment that the `Launcher` reads and
/// writes. A test gives temporary directories.
#[derive(Debug, Clone)]
pub struct Dirs {
    /// The home directory: `~` in a typed path and in the settings.
    pub home: PathBuf,
    /// The settings directory with `settings.toml` and `shortcuts.toml`.
    pub config: PathBuf,
    /// The cache directory: the stored file index and the currency rates.
    pub cache: PathBuf,
    /// The state directory: the start counts of the applications.
    pub state: PathBuf,
    /// The environment of the application list.
    pub applications: Environment,
    /// The time zone database with `zone.tab` and `iso3166.tab`.
    pub zoneinfo: PathBuf,
    /// The city list of libgweather. A missing file is not an error.
    pub city_list: PathBuf,
}

impl Dirs {
    /// The directories of the XDG Base Directory rules for this process:
    /// `$XDG_CONFIG_HOME/petlauncher`, `$XDG_CACHE_HOME/petlauncher` and
    /// `$XDG_STATE_HOME/petlauncher`, with `~/.config`, `~/.cache` and
    /// `~/.local/state` when a variable is not an absolute path. `None`
    /// without an absolute `HOME`.
    pub fn from_env() -> Option<Self> {
        let absolute = |name: &str| {
            std::env::var_os(name)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        let home = absolute("HOME")?;
        let base = |name: &str, default: &str| absolute(name).unwrap_or_else(|| home.join(default));
        Some(Self {
            config: base("XDG_CONFIG_HOME", ".config").join("petlauncher"),
            cache: base("XDG_CACHE_HOME", ".cache").join("petlauncher"),
            state: base("XDG_STATE_HOME", ".local/state").join("petlauncher"),
            applications: Environment::from_vars(&home, |name| std::env::var(name).ok()),
            zoneinfo: absolute("TZDIR").unwrap_or_else(|| PathBuf::from("/usr/share/zoneinfo")),
            city_list: PathBuf::from(GWEATHER_LOCATIONS),
            home,
        })
    }
}

/// The functions and values that the program gives to the `Launcher`.
/// `Hooks::new` sets the values for the program. A test changes the fields.
pub struct Hooks {
    /// The rule that makes an entry a settings page. The desktop part gives
    /// it, for example `petlauncher_apps::gnome::is_settings_page`.
    pub settings_page: SettingsPageRule,
    /// Called on a background thread when the rows of a query can be
    /// different now. Different threads can call it at the same time. The
    /// program sends the update to its main loop and searches again.
    pub on_change: OnChange,
    /// The start at login. `None`: the `Launcher` does not touch the
    /// autostart file.
    pub autostart: Option<Autostart>,
    /// Watch the settings directory and read a changed file again (R18).
    pub watch_config: bool,
    /// The wall clock for the kept query, the time queries, the rescan rule
    /// and the rates rule.
    pub clock: Arc<dyn Clock>,
    /// The time zone of the owner.
    pub time_zone: TimeZone,
    /// The download of the rates. It runs only on the rates thread.
    pub fetch_rates: FetchRates,
    /// The file index obeys the global git excludes file and the ignore
    /// files above its roots. A test sets false.
    pub outside_ignore_files: bool,
}

impl Hooks {
    /// The system clock, the time zone of the system, the ECB download and
    /// a settings watch, with no autostart file.
    pub fn new(
        settings_page: SettingsPageRule,
        on_change: impl Fn(Update) + Send + Sync + 'static,
    ) -> Self {
        Self {
            settings_page,
            on_change: Arc::new(on_change),
            autostart: None,
            watch_config: true,
            clock: Arc::new(SystemClock),
            time_zone: TimeZone::system(),
            fetch_rates: Arc::new(|| petlauncher_calc::fetch_ecb(FETCH_TIMEOUT)),
            outside_ignore_files: true,
        }
    }
}

/// How the program opens a URL in the browser of a shortcut (R12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserStart {
    /// Start this desktop entry with the URL, for example with
    /// `gio::DesktopAppInfo::launch_uris`. `command` is the `Exec` argument
    /// list with the URL, for a program with no GIO.
    DesktopEntry { path: PathBuf, command: Vec<String> },
    /// Run this argument list. The URL is the last argument.
    Command(Vec<String>),
}

/// The settings, the engine with all providers, the kept query, the usage
/// counts and the background threads. The program holds one `Launcher` in
/// an `Arc`. `search` runs on a worker thread (ADR 0002), the other methods
/// on the thread of the user interface. Drop stops the threads.
pub struct Launcher {
    shared: Arc<Shared>,
    watcher: Mutex<Option<ConfigWatcher>>,
    loader_jobs: Option<Sender<LoaderJob>>,
    loader: Option<JoinHandle<()>>,
    rates_commands: Option<Sender<RatesCommand>>,
    rates: Option<JoinHandle<()>>,
}

struct Shared {
    engine: Engine,
    config: Mutex<ConfigStore>,
    snapshot: RwLock<Snapshot>,
    kept: Mutex<KeptQuery>,
    clock: Arc<dyn Clock>,
    apps: Arc<AppsState>,
    index: Arc<FileIndex>,
    home: PathBuf,
    outside_ignore_files: bool,
    scope_error: Mutex<Option<ScopeError>>,
    last_rescan: Mutex<SystemTime>,
    dictionary: Arc<DictionaryState>,
    autostart: Option<Autostart>,
    pending: Arc<Pending>,
    problems: Arc<Problems>,
    rates_flags: Arc<RatesFlags>,
}

/// The settings and shortcuts in effect. A search takes a copy, so that a
/// reload does not wait for a search.
#[derive(Clone)]
struct Snapshot {
    settings: Arc<Settings>,
    shortcuts: Arc<[Shortcut]>,
}

impl Snapshot {
    fn of(store: &ConfigStore) -> Self {
        Self {
            settings: Arc::new(store.settings().clone()),
            shortcuts: store.shortcuts().into(),
        }
    }
}

impl Launcher {
    /// Reads the settings, the stored file index, the applications, the
    /// usage counts and the stored rates, and starts the background work:
    /// the city list, a scan of the file index and a check of the rates.
    /// The error is a thread that the system cannot start.
    pub fn start(dirs: Dirs, hooks: Hooks) -> io::Result<Self> {
        let Hooks {
            settings_page,
            on_change,
            autostart,
            watch_config,
            clock,
            time_zone,
            fetch_rates,
            outside_ignore_files,
        } = hooks;
        let store = ConfigStore::open(&dirs.config);
        let snapshot = Snapshot::of(&store);
        let settings = Arc::clone(&snapshot.settings);
        let problems = Arc::new(Problems::default());
        let pending = Arc::new(Pending::default());

        let usage = UsageStore::open(&dirs.state)
            .map_err(|error| {
                problems.set(
                    Source::Usage,
                    Some(format!(
                        "The start counts of the applications are not used: {}: {error}",
                        dirs.state.join(UsageStore::FILE_NAME).display()
                    )),
                );
            })
            .ok();
        let apps = Arc::new(AppsState::new(
            Applications::load(dirs.applications.clone(), settings_page),
            usage,
        ));
        let (index, scope_error) = open_index(
            &dirs,
            &settings.file_search,
            outside_ignore_files,
            &on_change,
        );
        index.rescan();
        let rates: SharedRates = Arc::new(RwLock::new(load_cache(&dirs.cache).ok().flatten()));
        let cities = Arc::new(OnceLock::new());
        let dictionary = Arc::new(DictionaryState::new(
            &dirs.home,
            &settings.dictionary.wordnet_path,
        ));

        let mut engine = Engine::new(&dirs.home);
        engine
            .register(Slot::Calculator, CalcProvider::new(Arc::clone(&rates)))
            .register(
                Slot::Time,
                TimeProvider::new(Arc::clone(&cities), Arc::clone(&clock), time_zone.clone()),
            )
            .register(Slot::Applications, ApplicationsProvider(Arc::clone(&apps)))
            .register(
                Slot::SettingsPages,
                SettingsPagesProvider(Arc::clone(&apps)),
            )
            .register(
                Slot::FileSearch,
                FileSearchProvider::new(Arc::clone(&index), dirs.home.clone()),
            )
            .register(
                Slot::Dictionary,
                DictionaryProvider(Arc::clone(&dictionary)),
            );

        let rates_flags = Arc::new(RatesFlags::default());
        let shared = Arc::new(Shared {
            engine,
            config: Mutex::new(store),
            snapshot: RwLock::new(snapshot),
            kept: Mutex::new(KeptQuery::new(Arc::clone(&clock))),
            clock: Arc::clone(&clock),
            apps: Arc::clone(&apps),
            index,
            home: dirs.home.clone(),
            outside_ignore_files,
            scope_error: Mutex::new(scope_error),
            last_rescan: Mutex::new(clock.now()),
            dictionary,
            autostart,
            pending: Arc::clone(&pending),
            problems: Arc::clone(&problems),
            rates_flags: Arc::clone(&rates_flags),
        });
        shared.update_autostart(&settings);

        let (loader_jobs, jobs) = mpsc::channel();
        let _ = loader_jobs.send(LoaderJob::LoadCities(pending.begin()));
        let loader = Loader {
            cities,
            zoneinfo: dirs.zoneinfo.clone(),
            city_list: dirs.city_list.clone(),
            apps,
            on_change: Arc::clone(&on_change),
            problems: Arc::clone(&problems),
        };
        let loader = thread::Builder::new()
            .name("petlauncher-loader".to_owned())
            .spawn(move || loader.run(jobs))?;

        let (rates_commands, commands) = mpsc::channel();
        let rates_job = RatesJob {
            rates,
            cache_dir: dirs.cache.clone(),
            fetch: fetch_rates,
            clock,
            time_zone,
            on_change: Arc::clone(&on_change),
            problems: Arc::clone(&problems),
            flags: rates_flags,
        };
        let first_check = pending.begin();
        let rates = thread::Builder::new()
            .name("petlauncher-rates".to_owned())
            .spawn(move || rates_job.run(first_check, commands))?;

        let watcher = if watch_config {
            watch(&shared, &dirs, on_change)
        } else {
            None
        };

        Ok(Self {
            shared,
            watcher: Mutex::new(watcher),
            loader_jobs: Some(loader_jobs),
            loader: Some(loader),
            rates_commands: Some(rates_commands),
            rates: Some(rates),
        })
    }

    /// The rows for the query. Call it on a worker thread, with a new token
    /// for each query (ADR 0002). It does not wait for the background work:
    /// a provider whose data is not loaded yet gives no rows.
    pub fn search(&self, query: &str, cancel: &CancelToken) -> Results {
        let snapshot = read(&self.shared.snapshot).clone();
        self.shared
            .engine
            .search(query, &snapshot.settings, &snapshot.shortcuts, cancel)
    }

    /// What `key` does on the `selected` row. See `petlauncher_core::activate`.
    /// The program executes the action, then calls [`Launcher::executed`].
    pub fn activate(&self, results: &Results, selected: Option<usize>, key: Key) -> Outcome {
        petlauncher_core::activate(results, selected, key)
    }

    /// The program executed `outcome` for `query`. A started desktop entry
    /// gets one more start in the usage counts. When `outcome.close` is
    /// true, this is also the close of the window for the kept query (R9):
    /// do not call `window_closed` for the same close. The error is a usage
    /// file that cannot be written. The count in memory has the start.
    pub fn executed(&self, query: &str, outcome: &Outcome) -> io::Result<()> {
        if outcome.close {
            self.window_closed(
                query,
                CloseReason::Activated {
                    keep_query: outcome.keep_query,
                },
            );
        }
        match &outcome.action {
            Action::LaunchDesktopEntry(path) => self.shared.apps.record_start(path),
            _ => Ok(()),
        }
    }

    /// The window opens. Gives the kept query (R9), or `None` for an empty
    /// query line. Also starts the background checks: a scan of the file
    /// index when the last scan started `RESCAN_AGE` or more ago, a check
    /// of the application files, and a check of the age of the rates. A
    /// dictionary that did not open tries again at the next query.
    pub fn window_opened(&self) -> Option<String> {
        let settings = self.settings();
        let query = lock(&self.shared.kept).window_opened(&settings);
        self.shared.rescan_when_old();
        if let Some(jobs) = &self.loader_jobs {
            let _ = jobs.send(LoaderJob::CheckApplications(self.shared.pending.begin()));
        }
        if let Some(commands) = &self.rates_commands {
            let _ = commands.send(RatesCommand::Check(self.shared.pending.begin()));
        }
        self.shared.dictionary.forget_failure();
        query
    }

    /// The window closed with no key on a row: a loss of focus, Escape or
    /// the global key (R9). For a close after a key on a row, use
    /// [`Launcher::executed`].
    pub fn window_closed(&self, query: &str, reason: CloseReason) {
        let settings = self.settings();
        lock(&self.shared.kept).window_closed(query, reason, &settings);
    }

    /// The settings in effect.
    pub fn settings(&self) -> Arc<Settings> {
        Arc::clone(&read(&self.shared.snapshot).settings)
    }

    /// The shortcuts in effect.
    pub fn shortcuts(&self) -> Arc<[Shortcut]> {
        Arc::clone(&read(&self.shared.snapshot).shortcuts)
    }

    /// Changes `settings.toml` and keeps the comments (R18, R21). After
    /// `Ok`, the new values are in effect, and the file index, the
    /// dictionary and the autostart file use them.
    pub fn edit_settings(&self, change: impl FnOnce(&mut SettingsEditor)) -> Result<(), EditError> {
        let result = {
            let mut store = lock(&self.shared.config);
            let result = store.edit_settings(change);
            *write(&self.shared.snapshot) = Snapshot::of(&store);
            result
        };
        self.shared.apply_settings();
        result
    }

    /// Changes `shortcuts.toml` and keeps the comments. See
    /// `ConfigStore::edit_shortcuts`.
    pub fn edit_shortcuts(
        &self,
        change: impl FnOnce(&mut ShortcutsEditor) -> Result<(), EditError>,
    ) -> Result<(), EditError> {
        let mut store = lock(&self.shared.config);
        let result = store.edit_shortcuts(change);
        // A `FileChanged` error reads the file again, so also an error can
        // change the values.
        *write(&self.shared.snapshot) = Snapshot::of(&store);
        result
    }

    /// Reads `file` again. The watcher does this when `Hooks::watch_config`
    /// is true. With an error in the file, the old values stay in effect
    /// and [`Launcher::config_error`] gives the error (R18).
    pub fn reload(&self, file: ConfigFile) -> Reload {
        self.shared.reload(file)
    }

    /// The error that keeps the last text of `file` out of effect.
    pub fn config_error(&self, file: ConfigFile) -> Option<ConfigError> {
        lock(&self.shared.config).error(file).cloned()
    }

    /// The warnings of the settings and shortcuts in effect.
    pub fn config_warnings(&self) -> Vec<Problem> {
        lock(&self.shared.config).warnings()
    }

    /// The error of the `[file_search]` settings that the file index cannot
    /// use, for example a pattern that is not correct. The index then uses
    /// the last scope that was correct, or the initial scope at the start.
    pub fn file_search_error(&self) -> Option<ScopeError> {
        lock(&self.shared.scope_error).clone()
    }

    /// Problems of the background work for the owner, as text: the city
    /// list, the usage file, the cache files, the autostart file and the
    /// settings watch. A failed download of the rates is not a problem.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = self.shared.problems.list();
        problems.extend(self.shared.index.last_error());
        problems
    }

    /// The meanings of a word for the dictionary window (R19). The first
    /// use of the dictionary opens it, which takes about 50 ms.
    pub fn dictionary_lookup(&self, word: &str) -> Result<Vec<Entry>, Arc<DictionaryError>> {
        self.shared.dictionary.lookup(word)
    }

    /// At most `limit` words that start with `text`, for the search field
    /// of the dictionary window.
    pub fn dictionary_suggestions(
        &self,
        text: &str,
        limit: usize,
    ) -> Result<Vec<String>, Arc<DictionaryError>> {
        self.shared.dictionary.suggestions(text, limit)
    }

    /// The installed browsers for the list of the settings window (R12), in
    /// the order of their names. The `id` of an entry is the value of
    /// `Browser::App`.
    pub fn browsers(&self) -> Vec<DesktopEntry> {
        self.shared
            .apps
            .list()
            .browsers()
            .into_iter()
            .cloned()
            .collect()
    }

    /// How to open `url` with the browser of a shortcut (R12). `None` when
    /// the desktop entry of `Browser::App` is not installed. A
    /// `Browser::Command` runs with `sh`, with the URL as its last argument.
    pub fn browser_start(&self, browser: &Browser, url: &str) -> Option<BrowserStart> {
        match browser {
            Browser::App(id) => {
                let list = self.shared.apps.list();
                let entry = list.get(id)?;
                Some(BrowserStart::DesktopEntry {
                    path: entry.path.clone(),
                    command: entry.command(Some(url)),
                })
            }
            Browser::Command(command) => Some(BrowserStart::Command(vec![
                "sh".to_owned(),
                "-c".to_owned(),
                format!("{command} \"$1\""),
                "sh".to_owned(),
                url.to_owned(),
            ])),
        }
    }

    /// Waits until the background work that started has ended: the city
    /// list, the checks of the applications and the rates, and the scan of
    /// the file index. For the tests and the example program.
    pub fn wait_idle(&self) {
        loop {
            self.shared.pending.wait();
            self.shared.index.wait();
            if self.shared.pending.is_idle() && !self.shared.index.is_scanning() {
                return;
            }
        }
    }
}

impl Drop for Launcher {
    /// Stops the watch and the threads. A scan of the file index stops at
    /// its next check when the index is dropped. A download of the rates
    /// that runs is not waited for: its thread ends after the download.
    fn drop(&mut self) {
        lock(&self.watcher).take();
        self.loader_jobs.take();
        if let Some(loader) = self.loader.take() {
            let _ = loader.join();
        }
        let flags = &self.shared.rates_flags;
        flags.stopped.store(true, Ordering::SeqCst);
        self.rates_commands.take();
        if let Some(rates) = self.rates.take() {
            if !flags.fetching.load(Ordering::SeqCst) {
                let _ = rates.join();
            }
        }
    }
}

impl Shared {
    fn reload(&self, file: ConfigFile) -> Reload {
        let result = {
            let mut store = lock(&self.config);
            let result = store.reload(file);
            if result == Reload::Changed {
                *write(&self.snapshot) = Snapshot::of(&store);
            }
            result
        };
        if result == Reload::Changed && file == ConfigFile::Settings {
            self.apply_settings();
        }
        result
    }

    /// Gives the settings in effect to the parts that do not read them at
    /// each search: the scope of the file index (R21 step 3), the WordNet
    /// directory and the autostart file. The keywords need nothing, because
    /// each search reads them.
    fn apply_settings(&self) {
        let settings = Arc::clone(&read(&self.snapshot).settings);
        let scope = files::scope(&settings.file_search, &self.home, self.outside_ignore_files);
        let result = self.index.set_scope(scope);
        *lock(&self.scope_error) = result.err();
        self.dictionary.set_path(&settings.dictionary.wordnet_path);
        self.update_autostart(&settings);
    }

    fn update_autostart(&self, settings: &Settings) {
        let Some(autostart) = &self.autostart else {
            return;
        };
        let problem = autostart
            .update(settings.run_in_background)
            .err()
            .map(|error| {
                format!(
                    "The start at login is not changed: {}: {error}",
                    autostart.path().display()
                )
            });
        self.problems.set(Source::Autostart, problem);
    }

    fn rescan_when_old(&self) {
        let now = self.clock.now();
        let mut last = lock(&self.last_rescan);
        let old = now
            .duration_since(*last)
            .map_or(true, |elapsed| elapsed >= RESCAN_AGE);
        if old && self.index.rescan() {
            *last = now;
        }
    }
}

/// Opens the file index with the scope of the settings. A scope with an
/// error gives the initial scope and the error.
fn open_index(
    dirs: &Dirs,
    file_search: &FileSearch,
    outside_ignore_files: bool,
    on_change: &OnChange,
) -> (Arc<FileIndex>, Option<ScopeError>) {
    let open = |file_search: &FileSearch| {
        let on_change = Arc::clone(on_change);
        FileIndex::open_with_listener(
            files::scope(file_search, &dirs.home, outside_ignore_files),
            &dirs.cache,
            move || on_change(Update::Files),
        )
    };
    match open(file_search) {
        Ok(index) => (Arc::new(index), None),
        Err(error) => {
            let index = open(&FileSearch::default()).expect("the initial scope is correct");
            (Arc::new(index), Some(error))
        }
    }
}

fn watch(shared: &Arc<Shared>, dirs: &Dirs, on_change: OnChange) -> Option<ConfigWatcher> {
    let weak = Arc::downgrade(shared);
    let watched = ConfigWatcher::start(&dirs.config, move |file| {
        let Some(shared) = weak.upgrade() else {
            return;
        };
        if shared.reload(file) != Reload::Unchanged {
            on_change(Update::Config(file));
        }
    });
    match watched {
        Ok(watcher) => Some(watcher),
        Err(error) => {
            shared.problems.set(
                Source::ConfigWatch,
                Some(format!(
                    "petlauncher does not see the changes of the settings files: {error}"
                )),
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn the_launcher_can_be_shared_between_threads() {
        assert_send_sync::<Launcher>();
    }
}
