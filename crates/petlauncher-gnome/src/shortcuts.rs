//! R4: the global key is a GNOME custom shortcut. GNOME keeps the list of
//! the custom shortcuts in `org.gnome.settings-daemon.plugins.media-keys
//! custom-keybindings`, and each shortcut in the relocatable schema
//! `org.gnome.settings-daemon.plugins.media-keys.custom-keybinding` at its
//! path. GNOME Settings shows the same entries.

use gio::prelude::*;
use petlauncher_core::DesktopError;

use crate::accelerator::{same_key, validate_accelerator};

/// The path of the custom shortcut of petlauncher.
pub const SHORTCUT_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/petlauncher/";
/// The name of the custom shortcut of petlauncher in GNOME Settings.
pub const SHORTCUT_NAME: &str = "Show petlauncher";
/// The command of the custom shortcut of petlauncher.
pub const TOGGLE_COMMAND: &str = "petlauncher toggle";

const LIST_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const LIST_KEY: &str = "custom-keybindings";
const ENTRY_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const ENTRY_KEYS: [&str; 3] = ["name", "command", "binding"];

/// One GNOME custom shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomShortcut {
    /// The GSettings path, for example
    /// `/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/ulauncher/`.
    pub path: String,
    pub name: String,
    pub command: String,
    /// The accelerator, for example `<Super>space`. Empty: no key.
    pub binding: String,
}

/// The GNOME custom shortcuts, read and written with GSettings.
pub struct CustomShortcuts {
    list: gio::Settings,
    entry_schema: gio::SettingsSchema,
    backend: Option<gio::SettingsBackend>,
}

impl CustomShortcuts {
    /// Uses the installed schemas and the default GSettings backend, which
    /// is dconf on GNOME.
    pub fn new() -> Result<Self, DesktopError> {
        let source = gio::SettingsSchemaSource::default()
            .ok_or_else(|| DesktopError("No GSettings schemas are installed.".into()))?;
        Self::open(&source, None)
    }

    /// Uses the given schemas and backend. A test gives the memory backend.
    pub fn with_backend(
        source: &gio::SettingsSchemaSource,
        backend: &gio::SettingsBackend,
    ) -> Result<Self, DesktopError> {
        Self::open(source, Some(backend))
    }

    fn open(
        source: &gio::SettingsSchemaSource,
        backend: Option<&gio::SettingsBackend>,
    ) -> Result<Self, DesktopError> {
        let list_schema = lookup(source, LIST_SCHEMA, &[LIST_KEY])?;
        let entry_schema = lookup(source, ENTRY_SCHEMA, &ENTRY_KEYS)?;
        Ok(Self {
            list: gio::Settings::new_full(&list_schema, backend, None),
            entry_schema,
            backend: backend.cloned(),
        })
    }

    /// The key of the petlauncher shortcut. `None` when the shortcut is not
    /// in the list of GNOME or has no key.
    pub fn global_key(&self) -> Option<String> {
        if !self.paths().iter().any(|path| path == SHORTCUT_PATH) {
            return None;
        }
        let binding = self.entry(SHORTCUT_PATH).string("binding");
        (!binding.is_empty() && binding != "disabled").then(|| binding.into())
    }

    /// Writes the petlauncher shortcut with `accelerator` and `command`, and
    /// adds it to the end of the list when it is not there. The other
    /// entries do not change, also when one of them has the same key: see
    /// `take_key`. An empty `accelerator` removes the key.
    pub fn set_global_key(&self, accelerator: &str, command: &str) -> Result<(), DesktopError> {
        validate_accelerator(accelerator).map_err(|error| DesktopError(error.to_string()))?;
        let entry = self.entry(SHORTCUT_PATH);
        entry.delay();
        for (key, value) in [
            ("name", SHORTCUT_NAME),
            ("command", command),
            ("binding", accelerator),
        ] {
            entry.set_string(key, value).map_err(write_error)?;
        }
        entry.apply();
        let mut paths = self.paths();
        if !paths.iter().any(|path| path == SHORTCUT_PATH) {
            paths.push(SHORTCUT_PATH.to_owned());
            self.list
                .set_strv(LIST_KEY, paths.as_slice())
                .map_err(write_error)?;
        }
        gio::Settings::sync();
        Ok(())
    }

    /// The other custom shortcuts in the list of GNOME that have the same
    /// key as `accelerator`. The settings window shows them as a warning.
    pub fn others_with_key(&self, accelerator: &str) -> Result<Vec<CustomShortcut>, DesktopError> {
        validate_accelerator(accelerator).map_err(|error| DesktopError(error.to_string()))?;
        Ok(self
            .paths()
            .iter()
            .filter(|path| path.as_str() != SHORTCUT_PATH && is_valid_path(path))
            .map(|path| self.read(path))
            .filter(|shortcut| same_key(&shortcut.binding, accelerator))
            .collect())
    }

    /// R4: removes `accelerator` from each other custom shortcut that has
    /// it, so that the key is free for petlauncher. The entries stay, with
    /// an empty key. Gives the changed entries with their old key.
    pub fn take_key(&self, accelerator: &str) -> Result<Vec<CustomShortcut>, DesktopError> {
        let taken = self.others_with_key(accelerator)?;
        for shortcut in &taken {
            self.entry(&shortcut.path)
                .set_string("binding", "")
                .map_err(write_error)?;
        }
        gio::Settings::sync();
        Ok(taken)
    }

    fn paths(&self) -> Vec<String> {
        self.list
            .strv(LIST_KEY)
            .iter()
            .map(|path| path.to_string())
            .collect()
    }

    fn entry(&self, path: &str) -> gio::Settings {
        gio::Settings::new_full(&self.entry_schema, self.backend.as_ref(), Some(path))
    }

    fn read(&self, path: &str) -> CustomShortcut {
        let entry = self.entry(path);
        let [name, command, binding] = ENTRY_KEYS.map(|key| entry.string(key).to_string());
        CustomShortcut {
            path: path.to_owned(),
            name,
            command,
            binding,
        }
    }
}

fn lookup(
    source: &gio::SettingsSchemaSource,
    id: &str,
    keys: &[&str],
) -> Result<gio::SettingsSchema, DesktopError> {
    let schema = source
        .lookup(id, true)
        .ok_or_else(|| DesktopError(format!("The GSettings schema {id} is not installed.")))?;
    match keys.iter().find(|key| !schema.has_key(key)) {
        Some(key) => Err(DesktopError(format!(
            "The GSettings schema {id} has no key {key}."
        ))),
        None => Ok(schema),
    }
}

/// GSettings accepts a path that starts and ends with `/` and has no `//`.
/// A different path in the list of GNOME is skipped, because GSettings
/// cannot open it.
fn is_valid_path(path: &str) -> bool {
    path.starts_with('/') && path.ends_with('/') && !path.contains("//")
}

fn write_error(error: glib::BoolError) -> DesktopError {
    DesktopError(format!(
        "Could not write the GNOME custom shortcut: {error}"
    ))
}
