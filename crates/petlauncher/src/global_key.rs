//! R4, first installation: petlauncher writes its global key one time.
//!
//! When GNOME has no custom shortcut of petlauncher, the program writes it
//! with the key of the setting `hotkey` and the command
//! `petlauncher toggle`, but only when no other GNOME key binding has that
//! key. The program never changes a different key binding. With a conflict
//! it writes nothing and shows a notice in the launcher window, and it tries
//! again at the next show and the next start.
//!
//! A marker file in the state directory records that the first installation
//! is done. While it exists, the program does not write the key again, so a
//! key that the owner removed in GNOME Settings stays removed.

use std::fs;
use std::io;
use std::path::Path;

use petlauncher_gnome::{Conflict, CustomShortcuts, SHORTCUTS_PAGE, TOGGLE_COMMAND};

/// The marker file in the state directory. Delete it to do the first
/// installation again at the next start.
pub const MARKER: &str = "global-key-installed";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FirstInstall {
    /// The marker file exists. Nothing changed.
    Done,
    /// GNOME has a key for petlauncher already. Nothing changed.
    KeyExists(String),
    /// The setting `hotkey` is empty. Nothing changed.
    NoKey,
    /// petlauncher has the key now.
    Installed(String),
    /// Other key bindings have the key. Nothing changed, and the marker is
    /// not written, so the program tries again.
    Conflicts {
        key: String,
        conflicts: Vec<Conflict>,
    },
}

impl FirstInstall {
    /// The line for the log.
    pub fn log_line(&self) -> String {
        match self {
            Self::Done => "the first installation of the global key is done already".into(),
            Self::KeyExists(key) => format!("GNOME has the global key {key} for petlauncher"),
            Self::NoKey => "the setting hotkey is empty, so no global key is set".into(),
            Self::Installed(key) => format!("the global key {key} runs \"{TOGGLE_COMMAND}\""),
            Self::Conflicts { key, conflicts } => {
                let names: Vec<String> = conflicts.iter().map(ToString::to_string).collect();
                format!(
                    "the global key {key} is not set, because it is the key of {}",
                    names.join(" and of ")
                )
            }
        }
    }

    /// The notice for the launcher window, for a conflict only: one line
    /// for each key binding with the key.
    pub fn notice(&self) -> Option<String> {
        let Self::Conflicts { key, conflicts } = self else {
            return None;
        };
        let mut text =
            format!("petlauncher has no global key. {key} is the key of these bindings in {SHORTCUTS_PAGE}:");
        for conflict in conflicts {
            match &conflict.section {
                Some(section) => text.push_str(&format!("\n• {section}: {}", conflict.name)),
                None => text.push_str(&format!("\n• {}, {}", conflict.name, conflict.place())),
            }
        }
        text.push_str(
            "\nRemove the key there, or with the settings window of petlauncher. \
             Then open this window again.",
        );
        Some(text)
    }
}

/// Does the first installation when the marker file in `state_dir` does
/// not exist. The error is for the log: a GSettings write or the marker
/// file failed, and the next show tries again.
pub fn install_once(
    shortcuts: &CustomShortcuts,
    hotkey: &str,
    state_dir: &Path,
) -> Result<FirstInstall, String> {
    let marker = state_dir.join(MARKER);
    if marker.exists() {
        return Ok(FirstInstall::Done);
    }
    let result = match shortcuts.global_key() {
        Some(key) => FirstInstall::KeyExists(key),
        None if hotkey.trim().is_empty() => FirstInstall::NoKey,
        None => {
            let conflicts = shortcuts
                .set_global_key_if_free(hotkey, TOGGLE_COMMAND)
                .map_err(|error| error.to_string())?;
            if !conflicts.is_empty() {
                return Ok(FirstInstall::Conflicts {
                    key: hotkey.to_owned(),
                    conflicts,
                });
            }
            FirstInstall::Installed(hotkey.to_owned())
        }
    };
    write_marker(&marker, &result).map_err(|error| format!("{}: {error}", marker.display()))?;
    Ok(result)
}

fn write_marker(marker: &Path, result: &FirstInstall) -> io::Result<()> {
    if let Some(dir) = marker.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(
        marker,
        format!(
            "The first installation of the global key of petlauncher is done (R4): {}.\n\
             petlauncher does not write the key again while this file exists.\n",
            result.log_line()
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::gio;
    use gtk::gio::prelude::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::{Once, OnceLock};

    const LIST: &str = "org.gnome.settings-daemon.plugins.media-keys";
    const ENTRY: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
    const WM: &str = "org.gnome.desktop.wm.keybindings";
    const NOTES: &str =
        "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/scratch-notes/";

    fn gnome_tests() -> PathBuf {
        [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "petlauncher-gnome",
            "tests",
        ]
        .iter()
        .collect()
    }

    /// The memory GSettings backend, after a check that the default backend
    /// of the test process is also the memory backend. No test reaches
    /// dconf.
    fn memory_backend() -> gio::SettingsBackend {
        static ISOLATE: Once = Once::new();
        ISOLATE.call_once(|| std::env::set_var("GSETTINGS_BACKEND", "memory"));
        assert_eq!(
            gio::SettingsBackend::default().type_().name(),
            "GMemorySettingsBackend"
        );
        gio::memory_settings_backend_new()
    }

    /// The test schemas of `petlauncher-gnome`, compiled once, with their
    /// XML files for the summaries of the keys.
    fn schemas() -> gio::SettingsSchemaSource {
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

    struct Fixture {
        backend: gio::SettingsBackend,
        source: gio::SettingsSchemaSource,
        state: tempfile::TempDir,
    }

    impl Fixture {
        /// A custom shortcut of a different program with Super+Space, and
        /// the input source switch with no key.
        fn new() -> Self {
            let fixture = Self {
                backend: memory_backend(),
                source: schemas(),
                state: tempfile::tempdir().unwrap(),
            };
            fixture
                .settings(LIST, None)
                .set_strv("custom-keybindings", [NOTES])
                .unwrap();
            let notes = fixture.settings(ENTRY, Some(NOTES));
            notes.set_string("name", "Show Scratch Notes").unwrap();
            notes.set_string("command", "scratch-notes").unwrap();
            notes.set_string("binding", "<Super>space").unwrap();
            fixture
                .settings(WM, None)
                .set_strv("switch-input-source", [""; 0])
                .unwrap();
            fixture
        }

        fn settings(&self, schema: &str, path: Option<&str>) -> gio::Settings {
            let schema = self.source.lookup(schema, false).unwrap();
            gio::Settings::new_full(&schema, Some(&self.backend), path)
        }

        fn shortcuts(&self) -> CustomShortcuts {
            CustomShortcuts::with_backend(&self.source, &self.backend)
                .unwrap()
                .with_catalog_dirs(vec![gnome_tests().join("keybindings")])
        }

        fn install(&self, hotkey: &str) -> Result<FirstInstall, String> {
            install_once(&self.shortcuts(), hotkey, self.state.path())
        }

        fn marker(&self) -> bool {
            self.state.path().join(MARKER).exists()
        }
    }

    #[test]
    fn a_conflict_writes_nothing_and_no_marker() {
        let fixture = Fixture::new();

        let result = fixture.install("<Super>space").unwrap();

        let FirstInstall::Conflicts { key, conflicts } = &result else {
            panic!("{result:?}");
        };
        assert_eq!(key, "<Super>space");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].name, "Show Scratch Notes");
        assert_eq!(fixture.shortcuts().global_key(), None);
        assert_eq!(
            fixture.settings(ENTRY, Some(NOTES)).string("binding"),
            "<Super>space"
        );
        assert!(!fixture.marker());
    }

    #[test]
    fn the_notice_names_the_shortcut_its_place_and_the_settings_window() {
        let fixture = Fixture::new();

        let notice = fixture.install("<Super>space").unwrap().notice().unwrap();

        assert!(
            notice.starts_with(
                "petlauncher has no global key. <Super>space is the key of these bindings in \
                 GNOME Settings > Keyboard > View and Customize Shortcuts:"
            ),
            "{notice}"
        );
        assert!(
            notice.contains("\n• Custom Shortcuts: Show Scratch Notes\n"),
            "{notice}"
        );
        assert!(notice.contains("settings window"), "{notice}");
    }

    #[test]
    fn the_key_is_written_when_no_binding_has_it() {
        let fixture = Fixture::new();
        fixture
            .settings(ENTRY, Some(NOTES))
            .set_string("binding", "")
            .unwrap();

        let result = fixture.install("<Super>space").unwrap();

        assert_eq!(result, FirstInstall::Installed("<Super>space".into()));
        assert_eq!(result.notice(), None);
        assert_eq!(
            fixture.shortcuts().global_key().as_deref(),
            Some("<Super>space")
        );
        assert!(fixture.marker());
    }

    #[test]
    fn the_gnome_default_input_source_key_is_a_conflict() {
        let fixture = Fixture::new();
        fixture.settings(WM, None).reset("switch-input-source");
        fixture
            .settings(ENTRY, Some(NOTES))
            .set_string("binding", "")
            .unwrap();

        let notice = fixture.install("<Super>space").unwrap().notice().unwrap();

        assert!(
            notice.contains("\n• Typing: Switch to next input source\n"),
            "{notice}"
        );
    }

    #[test]
    fn a_key_that_the_owner_removed_later_is_not_written_again() {
        let fixture = Fixture::new();
        fixture.install("<Super>F12").unwrap();
        fixture
            .shortcuts()
            .set_global_key("", TOGGLE_COMMAND)
            .unwrap();

        let second = fixture.install("<Super>F12").unwrap();

        assert_eq!(second, FirstInstall::Done);
        assert_eq!(fixture.shortcuts().global_key(), None);
    }

    #[test]
    fn an_existing_key_of_petlauncher_stays_and_the_marker_is_written() {
        let fixture = Fixture::new();
        fixture
            .shortcuts()
            .set_global_key("<Super>F12", TOGGLE_COMMAND)
            .unwrap();

        let result = fixture.install("<Super>space").unwrap();

        assert_eq!(result, FirstInstall::KeyExists("<Super>F12".into()));
        assert!(fixture.marker());
    }

    #[test]
    fn an_empty_hotkey_sets_no_key_and_writes_the_marker() {
        let fixture = Fixture::new();

        let result = fixture.install(" ").unwrap();

        assert_eq!(result, FirstInstall::NoKey);
        assert_eq!(fixture.shortcuts().global_key(), None);
        assert!(fixture.marker());
    }

    #[test]
    fn a_wrong_hotkey_is_an_error_and_writes_nothing() {
        let fixture = Fixture::new();

        let error = fixture.install("<Win>space").unwrap_err();

        assert!(error.contains("<Win>"), "{error}");
        assert_eq!(fixture.shortcuts().global_key(), None);
        assert!(!fixture.marker());
    }

    #[test]
    fn the_log_lines_tell_what_happened() {
        let conflict = FirstInstall::Conflicts {
            key: "<Super>space".into(),
            conflicts: Vec::new(),
        };

        assert!(FirstInstall::Done.log_line().contains("done already"));
        assert!(FirstInstall::NoKey.log_line().contains("empty"));
        assert!(FirstInstall::KeyExists("<Alt>F1".into())
            .log_line()
            .contains("<Alt>F1"));
        assert!(FirstInstall::Installed("<Alt>F1".into())
            .log_line()
            .contains("petlauncher toggle"));
        assert!(conflict.log_line().contains("is not set"));
    }
}
