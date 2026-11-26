//! R4: the global key is a GNOME custom shortcut. GNOME keeps the list of
//! the custom shortcuts in `org.gnome.settings-daemon.plugins.media-keys
//! custom-keybindings`, and each shortcut in the relocatable schema
//! `org.gnome.settings-daemon.plugins.media-keys.custom-keybinding` at its
//! path. GNOME Settings shows the same entries.

use std::path::PathBuf;

use gio::prelude::*;
use petlauncher_core::DesktopError;

use crate::accelerator::{same_key, validate_accelerator};
use crate::bindings::{
    read_catalog, system_catalog_dirs, BindingKind, Conflict, BUILT_IN_SCHEMAS, CUSTOM_SECTION,
    LIST_KEY,
};

/// The path of the custom shortcut of petlauncher.
pub const SHORTCUT_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/petlauncher/";
/// The name of the custom shortcut of petlauncher in GNOME Settings.
pub const SHORTCUT_NAME: &str = "Show petlauncher";
/// The command of the custom shortcut of petlauncher.
pub const TOGGLE_COMMAND: &str = "petlauncher toggle";

const LIST_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const ENTRY_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
const ENTRY_KEYS: [&str; 3] = ["name", "command", "binding"];

/// One GNOME custom shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomShortcut {
    /// The GSettings path, for example
    /// `/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/`.
    pub path: String,
    pub name: String,
    pub command: String,
    /// The accelerator, for example `<Super>space`. Empty: no key.
    pub binding: String,
}

/// The GNOME custom shortcuts, read and written with GSettings, and the
/// built-in key bindings for the conflict check of R4.
pub struct CustomShortcuts {
    list: gio::Settings,
    entry_schema: gio::SettingsSchema,
    source: gio::SettingsSchemaSource,
    backend: Option<gio::SettingsBackend>,
    catalog_dirs: Vec<PathBuf>,
}

impl CustomShortcuts {
    /// Uses the installed schemas, the default GSettings backend, which is
    /// dconf on GNOME, and the files of GNOME Settings for the names of the
    /// key bindings.
    pub fn new() -> Result<Self, DesktopError> {
        let source = gio::SettingsSchemaSource::default()
            .ok_or_else(|| DesktopError("No GSettings schemas are installed.".into()))?;
        Ok(Self::open(&source, None)?.with_catalog_dirs(system_catalog_dirs()))
    }

    /// Uses the given schemas and backend, and no files of GNOME Settings.
    /// A test gives the memory backend.
    pub fn with_backend(
        source: &gio::SettingsSchemaSource,
        backend: &gio::SettingsBackend,
    ) -> Result<Self, DesktopError> {
        Self::open(source, Some(backend))
    }

    /// The directories with the files `*.xml` of GNOME Settings that give
    /// the names and sections of the built-in key bindings.
    pub fn with_catalog_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.catalog_dirs = dirs;
        self
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
            source: source.clone(),
            backend: backend.cloned(),
            catalog_dirs: Vec::new(),
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
    /// `set_global_key_if_free`. An empty `accelerator` removes the key.
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

    /// R4: each GNOME key binding that has the key `accelerator`: the other
    /// custom shortcuts, and the built-in bindings of the schemas that GNOME
    /// Settings shows. Two texts of one key are the same key, for example
    /// `<Super>space` and `<Mod4>space`. Empty for an empty `accelerator`.
    pub fn conflicts(&self, accelerator: &str) -> Result<Vec<Conflict>, DesktopError> {
        let custom = self.others_with_key(accelerator)?;
        if is_no_key(accelerator) {
            return Ok(Vec::new());
        }
        let mut conflicts: Vec<Conflict> = custom
            .into_iter()
            .map(|shortcut| Conflict {
                kind: BindingKind::Custom {
                    path: shortcut.path,
                },
                name: shortcut.name,
                section: Some(CUSTOM_SECTION.to_owned()),
                accelerator: shortcut.binding,
            })
            .collect();
        let catalog = read_catalog(&self.catalog_dirs);
        for schema_id in BUILT_IN_SCHEMAS {
            let Some(schema) = self.source.lookup(schema_id, true) else {
                continue;
            };
            let settings = gio::Settings::new_full(&schema, self.backend.as_ref(), None);
            let mut keys: Vec<String> = schema
                .list_keys()
                .iter()
                .map(|key| key.to_string())
                .collect();
            keys.sort();
            for key in keys {
                if !is_binding_key(&schema, &key) {
                    continue;
                }
                for stored in settings.strv(&key) {
                    if !same_key(&stored, accelerator) {
                        continue;
                    }
                    let shown = catalog
                        .iter()
                        .find(|entry| entry.schema == schema_id && entry.key == key);
                    conflicts.push(Conflict {
                        name: match shown {
                            Some(entry) => entry.description.clone(),
                            None => summary(&schema, &key),
                        },
                        section: shown.map(|entry| entry.section.clone()),
                        kind: BindingKind::BuiltIn {
                            schema: schema_id.to_owned(),
                            key: key.clone(),
                        },
                        accelerator: stored.to_string(),
                    });
                }
            }
        }
        Ok(conflicts)
    }

    /// R4: writes the petlauncher shortcut, as `set_global_key` does, only
    /// when no other key binding has the key. Gives the conflicts, and then
    /// writes nothing. Empty: the key is written.
    pub fn set_global_key_if_free(
        &self,
        accelerator: &str,
        command: &str,
    ) -> Result<Vec<Conflict>, DesktopError> {
        let conflicts = self.conflicts(accelerator)?;
        if conflicts.is_empty() {
            self.set_global_key(accelerator, command)?;
        }
        Ok(conflicts)
    }

    /// R4: removes the key of one conflict, after an explicit action of the
    /// owner. A custom shortcut keeps its name and command with an empty
    /// key, which GNOME Settings shows as disabled. A built-in binding loses
    /// only `conflict.accelerator`, and its other accelerators stay. No
    /// entry is removed. A key that is not there any more is not an error.
    pub fn remove_key(&self, conflict: &Conflict) -> Result<(), DesktopError> {
        match &conflict.kind {
            BindingKind::Custom { path } => {
                if path == SHORTCUT_PATH || !is_valid_path(path) {
                    return Err(DesktopError(format!(
                        "{path} is not the path of a different custom shortcut."
                    )));
                }
                let entry = self.entry(path);
                if entry.string("binding") == conflict.accelerator {
                    entry.set_string("binding", "").map_err(write_error)?;
                }
            }
            BindingKind::BuiltIn { schema, key } => {
                let found = BUILT_IN_SCHEMAS
                    .contains(&schema.as_str())
                    .then(|| self.source.lookup(schema, true))
                    .flatten()
                    .filter(|found| found.has_key(key) && is_binding_key(found, key))
                    .ok_or_else(|| {
                        DesktopError(format!("{schema} {key} is not a GNOME key binding."))
                    })?;
                let settings = gio::Settings::new_full(&found, self.backend.as_ref(), None);
                let old = settings.strv(key);
                let kept: Vec<&str> = old
                    .iter()
                    .map(|stored| stored.as_str())
                    .filter(|stored| *stored != conflict.accelerator)
                    .collect();
                if kept.len() != old.len() {
                    settings
                        .set_strv(key, kept.as_slice())
                        .map_err(write_error)?;
                }
            }
        }
        gio::Settings::sync();
        Ok(())
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

/// A key of a built-in schema that holds accelerators: type `as`, and not
/// the list of the custom shortcuts.
fn is_binding_key(schema: &gio::SettingsSchema, key: &str) -> bool {
    key != LIST_KEY && schema.key(key).value_type().as_str() == "as"
}

fn summary(schema: &gio::SettingsSchema, key: &str) -> String {
    schema
        .key(key)
        .summary()
        .map(|summary| summary.to_string())
        .unwrap_or_else(|| key.to_owned())
}

fn is_no_key(accelerator: &str) -> bool {
    accelerator.is_empty() || accelerator == "disabled"
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
