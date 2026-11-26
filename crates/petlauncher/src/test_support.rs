//! Fixtures of the unit tests: a `Backend` on temporary directories, and
//! the GNOME key bindings in the memory GSettings backend. No test reads the
//! real home directory, the real settings or dconf.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Once, OnceLock};

use gtk::gio;
use gtk::gio::prelude::*;
use petlauncher_calc::RatesError;
use petlauncher_core::DesktopError;
use petlauncher_gnome::CustomShortcuts;
use petlauncher_providers::{Dirs, Environment};

use crate::backend::{Backend, Events};

pub const LIST: &str = "org.gnome.settings-daemon.plugins.media-keys";
pub const ENTRY: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
pub const WM: &str = "org.gnome.desktop.wm.keybindings";
/// A custom shortcut of an invented program.
pub const NOTES: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/scratch-notes/";

/// A `Backend` with a `Launcher` on a temporary home, with no settings
/// watch, no download and no desktop.
pub struct LauncherFixture {
    _root: tempfile::TempDir,
    pub dirs: Dirs,
}

impl LauncherFixture {
    pub fn new() -> Self {
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

    pub fn start(&self) -> (Backend, Events) {
        let (backend, events) = Backend::start_with(
            self.dirs.clone(),
            Err(DesktopError("no desktop in the test".into())),
            |hooks| {
                hooks.watch_config = false;
                hooks.outside_ignore_files = false;
                hooks.fetch_rates = Arc::new(|| Err(RatesError::Http("no network".into())));
            },
        )
        .unwrap();
        backend.launcher().wait_idle();
        (backend, events)
    }

    pub fn home(&self, relative: &str) -> PathBuf {
        self.dirs.home.join(relative)
    }

    pub fn settings_file(&self) -> PathBuf {
        self.dirs.config.join("settings.toml")
    }

    pub fn shortcuts_file(&self) -> PathBuf {
        self.dirs.config.join("shortcuts.toml")
    }
}

/// The memory GSettings backend, after a check that the default backend of
/// the test process is also the memory backend.
pub fn memory_backend() -> gio::SettingsBackend {
    static ISOLATE: Once = Once::new();
    ISOLATE.call_once(|| std::env::set_var("GSETTINGS_BACKEND", "memory"));
    assert_eq!(
        gio::SettingsBackend::default().type_().name(),
        "GMemorySettingsBackend"
    );
    gio::memory_settings_backend_new()
}

/// The tests directory of `petlauncher-gnome`: its test schemas and its
/// copy of the files of GNOME Settings.
pub fn gnome_tests() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "petlauncher-gnome",
        "tests",
    ]
    .iter()
    .collect()
}

/// The test schemas of `petlauncher-gnome`, compiled once, with their XML
/// files for the summaries of the keys.
pub fn schemas() -> gio::SettingsSchemaSource {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        let sources = gnome_tests().join("schemas");
        let status = Command::new("glib-compile-schemas")
            .arg(format!("--targetdir={}", dir.path().display()))
            .arg(&sources)
            .status()
            .expect("glib-compile-schemas runs");
        assert!(status.success());
        for file in fs::read_dir(&sources).unwrap() {
            let path = file.unwrap().path();
            fs::copy(&path, dir.path().join(path.file_name().unwrap())).unwrap();
        }
        dir
    });
    gio::SettingsSchemaSource::from_directory(dir.path(), None, true).unwrap()
}

/// The GNOME key bindings in the memory backend, and a state directory.
pub struct SettingsBackendFixture {
    pub backend: gio::SettingsBackend,
    pub source: gio::SettingsSchemaSource,
    pub state: tempfile::TempDir,
}

impl SettingsBackendFixture {
    /// The GNOME defaults: the input source switch has Super+Space.
    pub fn new() -> Self {
        Self {
            backend: memory_backend(),
            source: schemas(),
            state: tempfile::tempdir().unwrap(),
        }
    }

    /// Two conflicts on Super+Space: a custom shortcut of an invented
    /// program and the GNOME default of the input source switch.
    pub fn with_conflicts() -> Self {
        let fixture = Self::new();
        fixture
            .settings(LIST, None)
            .set_strv("custom-keybindings", [NOTES])
            .unwrap();
        let notes = fixture.settings(ENTRY, Some(NOTES));
        notes.set_string("name", "Show Scratch Notes").unwrap();
        notes.set_string("command", "scratch-notes").unwrap();
        notes.set_string("binding", "<Super>space").unwrap();
        fixture
    }

    /// Only the custom shortcut has Super+Space, as on the desktop of the
    /// owner.
    pub fn with_one_conflict() -> Self {
        let fixture = Self::with_conflicts();
        fixture
            .settings(WM, None)
            .set_strv("switch-input-source", [""; 0])
            .unwrap();
        fixture
    }

    pub fn settings(&self, schema: &str, path: Option<&str>) -> gio::Settings {
        let schema = self.source.lookup(schema, false).unwrap();
        gio::Settings::new_full(&schema, Some(&self.backend), path)
    }

    pub fn shortcuts(&self) -> CustomShortcuts {
        CustomShortcuts::with_backend(&self.source, &self.backend)
            .unwrap()
            .with_catalog_dirs(vec![gnome_tests().join("keybindings")])
    }

    pub fn binding(&self, path: &str) -> String {
        self.settings(ENTRY, Some(path))
            .string("binding")
            .to_string()
    }

    pub fn input_source_keys(&self) -> Vec<String> {
        self.settings(WM, None)
            .strv("switch-input-source")
            .iter()
            .map(|key| key.to_string())
            .collect()
    }

    pub fn state_dir(&self) -> &Path {
        self.state.path()
    }
}
