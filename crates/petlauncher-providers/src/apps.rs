//! The application rows and the settings page rows (R2, R3, R13, R16).

use std::io;
use std::path::Path;
use std::sync::{Arc, RwLock};

use petlauncher_apps::{Applications, DesktopEntry, EntryKind, UsageCounts, UsageStore};
use petlauncher_core::config::Settings;
use petlauncher_core::{Action, CancelToken, Icon, Provider, Row, RowKind};

use crate::sync::{read, write};

/// The largest number of application rows in the general search.
const APPLICATION_ROWS: usize = 9;

/// The largest number of settings page rows in the general search.
const SETTINGS_PAGE_ROWS: usize = 5;

/// The list of desktop entries and the start counts. The loader thread
/// replaces the list after a change of the files. The program records a
/// start through `Launcher::executed`.
pub(crate) struct AppsState {
    list: RwLock<Arc<Applications>>,
    /// `None` when the usage file cannot be read. Then the rank has no usage
    /// points and no start is recorded.
    usage: RwLock<Option<UsageStore>>,
}

impl AppsState {
    pub(crate) fn new(list: Applications, usage: Option<UsageStore>) -> Self {
        Self {
            list: RwLock::new(Arc::new(list)),
            usage: RwLock::new(usage),
        }
    }

    pub(crate) fn list(&self) -> Arc<Applications> {
        Arc::clone(&read(&self.list))
    }

    pub(crate) fn replace(&self, list: Applications) {
        *write(&self.list) = Arc::new(list);
    }

    /// Adds one start of the entry of this `.desktop` file. Does nothing
    /// when the file is not an entry of the list.
    pub(crate) fn record_start(&self, desktop_file: &Path) -> io::Result<()> {
        let list = self.list();
        let Some(entry) = list
            .entries()
            .iter()
            .find(|entry| entry.path == desktop_file)
        else {
            return Ok(());
        };
        match write(&self.usage).as_mut() {
            Some(usage) => usage.record_start(&entry.id),
            None => Ok(()),
        }
    }

    /// The rows of the entries of one kind. The search ranks the
    /// applications and the settings pages together, so that a typing error
    /// shows only when no entry of the two kinds matches without one.
    fn rows(&self, text: &str, kind: EntryKind, limit: usize) -> Vec<Row> {
        let list = self.list();
        let usage = read(&self.usage);
        let no_usage = UsageCounts::new();
        let counts = usage.as_ref().map_or(&no_usage, UsageStore::counts);
        list.search(text, usize::MAX, counts)
            .into_iter()
            .filter(|hit| hit.entry.kind == kind)
            .take(limit)
            .map(|hit| entry_row(hit.entry))
            .collect()
    }
}

/// The installed applications, in `Slot::Applications`.
pub(crate) struct ApplicationsProvider(pub Arc<AppsState>);

impl Provider for ApplicationsProvider {
    fn search(&self, text: &str, _settings: &Settings, _cancel: &CancelToken) -> Vec<Row> {
        self.0.rows(text, EntryKind::Application, APPLICATION_ROWS)
    }
}

/// The settings pages, in `Slot::SettingsPages`. They come from the same
/// list as the applications, with the rule that the caller gave to
/// `Applications::load`. They stay in the search when "Include applications
/// in search" is off.
pub(crate) struct SettingsPagesProvider(pub Arc<AppsState>);

impl Provider for SettingsPagesProvider {
    fn search(&self, text: &str, _settings: &Settings, _cancel: &CancelToken) -> Vec<Row> {
        self.0
            .rows(text, EntryKind::SettingsPage, SETTINGS_PAGE_ROWS)
    }
}

fn entry_row(entry: &DesktopEntry) -> Row {
    let (kind, subtitle) = match entry.kind {
        EntryKind::Application => (
            RowKind::Application,
            entry
                .comment
                .clone()
                .or_else(|| entry.generic_name.clone())
                .unwrap_or_default(),
        ),
        EntryKind::SettingsPage => (RowKind::SettingsPage, "Settings".to_owned()),
    };
    Row::new(
        kind,
        entry.name.clone(),
        Action::LaunchDesktopEntry(entry.path.clone()),
    )
    .with_subtitle(subtitle)
    .with_icon(entry_icon(entry))
    .with_local_file(entry.path.clone())
}

fn entry_icon(entry: &DesktopEntry) -> Icon {
    match entry.icon.as_deref() {
        Some(path) if path.starts_with('/') => Icon::Path(path.into()),
        Some(name) => Icon::Name(name.to_owned()),
        None => Icon::Name("application-x-executable".to_owned()),
    }
}
