use std::path::{Path, PathBuf};

use crate::edit::{SettingsEditor, ShortcutsEditor};
use crate::error::{ConfigError, EditError};
use crate::load::{add_lines, parse_settings, parse_shortcuts, read_text, Loaded};
use crate::model::{Settings, Shortcut};
use crate::paths::ConfigFile;
use crate::validate::{validate_across, Problem};
use crate::write::write_atomic;

/// The result of [`ConfigStore::reload`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reload {
    /// The text of the file is the same as at the last read or write, for
    /// example after a write by petlauncher itself. Nothing changed.
    Unchanged,
    /// The new text is in effect. Show the new values and remove the error
    /// of the file, if there was one.
    Changed,
    /// The new text has an error. The old values stay in effect. Show
    /// [`ConfigStore::error`].
    Failed,
}

/// The settings and the shortcuts that are in effect.
///
/// The store reads the two files from a settings directory. When a file has
/// an error, the store keeps the last values that were good and keeps the
/// error, so the program can show it (R18). All changes from the settings
/// window go through the store, so that the store knows the text that
/// petlauncher wrote itself: a reload after its own write gives
/// [`Reload::Unchanged`].
#[derive(Debug)]
pub struct ConfigStore {
    dir: PathBuf,
    settings: FileState<Settings>,
    shortcuts: FileState<Vec<Shortcut>>,
}

#[derive(Debug)]
struct FileState<T> {
    value: T,
    warnings: Vec<Problem>,
    error: Option<ConfigError>,
    /// The text of the last read or write. `Some(None)`: the file did not
    /// exist. `None`: the last read failed.
    seen: Option<Option<String>>,
    /// The text that gave `value`. `None`: the initial values.
    in_effect: Option<String>,
}

impl ConfigStore {
    /// Reads the two files in `dir`. A file with an error gives the initial
    /// values and the error.
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        let mut store = Self {
            dir: dir.into(),
            settings: FileState::new(),
            shortcuts: FileState::new(),
        };
        for file in ConfigFile::ALL {
            store.reload(file);
        }
        store
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn settings(&self) -> &Settings {
        &self.settings.value
    }

    pub fn shortcuts(&self) -> &[Shortcut] {
        &self.shortcuts.value
    }

    /// The cause that the last text of `file` is not in effect.
    pub fn error(&self, file: ConfigFile) -> Option<&ConfigError> {
        match file {
            ConfigFile::Settings => self.settings.error.as_ref(),
            ConfigFile::Shortcuts => self.shortcuts.error.as_ref(),
        }
    }

    /// The warnings of the values in effect, with the conflicts between
    /// the two files.
    pub fn warnings(&self) -> Vec<Problem> {
        let mut warnings = self.settings.warnings.clone();
        warnings.extend(self.shortcuts.warnings.iter().cloned());
        let mut across = validate_across(&self.settings.value, &self.shortcuts.value);
        if let Some(text) = &self.shortcuts.in_effect {
            add_lines(text, &mut across);
        }
        warnings.extend(across);
        warnings
    }

    /// Reads `file` again, for example after a [`crate::ConfigWatcher`]
    /// reported a change.
    pub fn reload(&mut self, file: ConfigFile) -> Reload {
        let path = file.path_in(&self.dir);
        match file {
            ConfigFile::Settings => self.settings.reload(&path, parse_settings),
            ConfigFile::Shortcuts => self.shortcuts.reload(&path, parse_shortcuts),
        }
    }

    /// Changes `settings.toml` with the setters of [`SettingsEditor`] and
    /// writes it. A missing file starts from the template. When the result
    /// has an error, nothing is written. After `Ok`, the store has the new
    /// values.
    pub fn edit_settings(
        &mut self,
        change: impl FnOnce(&mut SettingsEditor),
    ) -> Result<(), EditError> {
        let path = ConfigFile::Settings.path_in(&self.dir);
        let mut editor = match read_text(&path)? {
            None => SettingsEditor::new_file(),
            Some(text) => SettingsEditor::parse(&text, &path)?,
        };
        let before = editor.text();
        change(&mut editor);
        let after = editor.text();
        if after == before {
            return Ok(());
        }
        let loaded = parse_settings(&after, &path).map_err(rejected)?;
        write_atomic(&path, &after)?;
        self.settings.accept(Some(after), loaded);
        Ok(())
    }

    /// Changes `shortcuts.toml` with [`ShortcutsEditor`] and writes it.
    /// When the file changed on the disk after the last reload, the store
    /// reads it again and gives [`EditError::FileChanged`], because the
    /// shortcut numbers can be different.
    pub fn edit_shortcuts(
        &mut self,
        change: impl FnOnce(&mut ShortcutsEditor) -> Result<(), EditError>,
    ) -> Result<(), EditError> {
        let path = ConfigFile::Shortcuts.path_in(&self.dir);
        let text = read_text(&path)?;
        if self.shortcuts.seen.as_ref() != Some(&text) {
            self.reload(ConfigFile::Shortcuts);
            return Err(EditError::FileChanged);
        }
        let mut editor = match text {
            None => ShortcutsEditor::new_file(),
            Some(text) => ShortcutsEditor::parse(&text, &path)?,
        };
        let before = editor.text();
        change(&mut editor)?;
        let after = editor.text();
        if after == before {
            return Ok(());
        }
        let loaded = parse_shortcuts(&after, &path).map_err(rejected)?;
        write_atomic(&path, &after)?;
        self.shortcuts.accept(Some(after), loaded);
        Ok(())
    }
}

/// An error in the result of a change: a validation error is the fault of
/// the change, other errors are the fault of the file.
fn rejected(error: ConfigError) -> EditError {
    match error {
        ConfigError::Invalid { problems, .. } => EditError::Invalid(problems),
        other => EditError::File(other),
    }
}

type Parse<T> = fn(&str, &Path) -> Result<Loaded<T>, ConfigError>;

impl<T: Default> FileState<T> {
    fn new() -> Self {
        Self {
            value: T::default(),
            warnings: Vec::new(),
            error: None,
            seen: None,
            in_effect: None,
        }
    }

    fn reload(&mut self, path: &Path, parse: Parse<T>) -> Reload {
        let text = match read_text(path) {
            Ok(text) => text,
            Err(error) => {
                self.seen = None;
                self.error = Some(error);
                return Reload::Failed;
            }
        };
        if self.seen.as_ref() == Some(&text) {
            return Reload::Unchanged;
        }
        self.seen = Some(text.clone());
        let result = match &text {
            None => Ok(Loaded {
                value: T::default(),
                warnings: Vec::new(),
            }),
            Some(text) => parse(text, path),
        };
        match result {
            Ok(loaded) => {
                self.accept(text, loaded);
                Reload::Changed
            }
            Err(error) => {
                self.error = Some(error);
                Reload::Failed
            }
        }
    }

    fn accept(&mut self, text: Option<String>, loaded: Loaded<T>) {
        self.seen = Some(text.clone());
        self.in_effect = text;
        self.value = loaded.value;
        self.warnings = loaded.warnings;
        self.error = None;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::model::KeepQuery;
    use crate::validate::{BuiltIn, ProblemKind, ShortcutRef};

    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().expect("temporary directory"),
            }
        }

        fn path(&self, file: ConfigFile) -> PathBuf {
            file.path_in(self.dir.path())
        }

        fn write(&self, file: ConfigFile, text: &str) {
            fs::write(self.path(file), text).expect("write test file");
        }

        fn read(&self, file: ConfigFile) -> Option<String> {
            fs::read_to_string(self.path(file)).ok()
        }

        fn store(&self) -> ConfigStore {
            ConfigStore::open(self.dir.path())
        }
    }

    const SHORTCUTS: &str =
        "[[shortcut]]\nname = \"Wiki\"\nkeyword = \"w\"\ncmd = \"https://wiki.example.com/%s\"\n";

    fn map_shortcut() -> Shortcut {
        Shortcut {
            name: "Map".to_owned(),
            keyword: "m".to_owned(),
            cmd: "https://maps.example.com/?q=%s".to_owned(),
            ..Shortcut::default()
        }
    }

    #[test]
    fn gives_the_initial_values_when_there_are_no_files() {
        let fixture = Fixture::new();

        let store = fixture.store();

        assert_eq!(store.settings(), &Settings::default());
        assert_eq!(store.shortcuts(), &[]);
        assert_eq!(store.error(ConfigFile::Settings), None);
        assert_eq!(store.error(ConfigFile::Shortcuts), None);
        assert_eq!(store.warnings(), vec![]);
        assert_eq!(store.dir(), fixture.dir.path());
    }

    #[test]
    fn gives_the_initial_values_and_the_error_of_a_broken_file_at_the_start() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Settings, "window_width = \"wide\"\n");

        let store = fixture.store();

        assert_eq!(store.settings(), &Settings::default());
        let error = store.error(ConfigFile::Settings).expect("error");
        assert_eq!(error.line(), Some(1));
        assert_eq!(error.path(), fixture.path(ConfigFile::Settings));
    }

    #[test]
    fn reads_a_changed_file() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        fixture.write(ConfigFile::Settings, "window_width = 900\n");
        fixture.write(ConfigFile::Shortcuts, SHORTCUTS);

        let settings = store.reload(ConfigFile::Settings);
        let shortcuts = store.reload(ConfigFile::Shortcuts);

        assert_eq!((settings, shortcuts), (Reload::Changed, Reload::Changed));
        assert_eq!(store.settings().window_width, 900);
        assert_eq!(store.shortcuts()[0].keyword, "w");
    }

    #[test]
    fn keeps_the_old_values_when_the_changed_file_has_an_error() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Settings, "window_width = 900\n");
        let mut store = fixture.store();
        fixture.write(ConfigFile::Settings, "window_width = 1000\nhotkey = \n");

        let result = store.reload(ConfigFile::Settings);

        assert_eq!(result, Reload::Failed);
        assert_eq!(store.settings().window_width, 900);
        assert_eq!(
            store
                .error(ConfigFile::Settings)
                .and_then(ConfigError::line),
            Some(2)
        );
    }

    #[test]
    fn keeps_the_old_values_when_the_changed_file_has_a_validation_error() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Shortcuts, SHORTCUTS);
        let mut store = fixture.store();
        fixture.write(
            ConfigFile::Shortcuts,
            &SHORTCUTS.replace("keyword = \"w\"", "keyword = \"\""),
        );

        let result = store.reload(ConfigFile::Shortcuts);

        assert_eq!(result, Reload::Failed);
        assert_eq!(store.shortcuts()[0].keyword, "w");
        assert!(matches!(
            store.error(ConfigFile::Shortcuts),
            Some(ConfigError::Invalid { .. })
        ));
    }

    #[test]
    fn removes_the_error_when_the_file_is_correct_again() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Settings, "window_width = \n");
        let mut store = fixture.store();
        fixture.write(ConfigFile::Settings, "window_width = 640\n");

        let result = store.reload(ConfigFile::Settings);

        assert_eq!(result, Reload::Changed);
        assert_eq!(store.error(ConfigFile::Settings), None);
        assert_eq!(store.settings().window_width, 640);
    }

    #[test]
    fn reports_no_change_when_the_text_is_the_same() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Settings, "window_width = 900\n");
        let mut store = fixture.store();

        assert_eq!(store.reload(ConfigFile::Settings), Reload::Unchanged);
        assert_eq!(store.reload(ConfigFile::Shortcuts), Reload::Unchanged);
    }

    #[test]
    fn gives_the_initial_values_when_the_file_is_removed() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Settings, "window_width = 900\n");
        let mut store = fixture.store();
        fs::remove_file(fixture.path(ConfigFile::Settings)).expect("remove");

        assert_eq!(store.reload(ConfigFile::Settings), Reload::Changed);
        assert_eq!(store.settings(), &Settings::default());
    }

    #[test]
    fn reports_a_read_error_and_keeps_the_values() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Settings, "window_width = 900\n");
        let mut store = fixture.store();
        fs::remove_file(fixture.path(ConfigFile::Settings)).expect("remove");
        fs::create_dir(fixture.path(ConfigFile::Settings)).expect("directory");

        assert_eq!(store.reload(ConfigFile::Settings), Reload::Failed);
        assert_eq!(store.settings().window_width, 900);
        assert!(matches!(
            store.error(ConfigFile::Settings),
            Some(ConfigError::Io { .. })
        ));
    }

    #[test]
    fn writes_a_change_and_does_not_reload_its_own_write() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Settings, "# mine\nwindow_width = 900\n");
        let mut store = fixture.store();

        store
            .edit_settings(|editor| editor.set_keep_query(KeepQuery::Forever))
            .expect("edit");

        assert_eq!(
            fixture.read(ConfigFile::Settings).as_deref(),
            Some("# mine\nwindow_width = 900\nkeep_query = \"forever\"\n")
        );
        assert_eq!(store.settings().keep_query, KeepQuery::Forever);
        assert_eq!(store.reload(ConfigFile::Settings), Reload::Unchanged);
    }

    #[test]
    fn makes_a_new_settings_file_from_the_template() {
        let fixture = Fixture::new();
        let mut store = fixture.store();

        store
            .edit_settings(|editor| editor.set_window_width(1200))
            .expect("edit");

        let text = fixture.read(ConfigFile::Settings).expect("new file");
        assert!(text.starts_with("# Settings of petlauncher."), "{text}");
        assert!(text.contains("\nwindow_width = 1200\n"), "{text}");
        assert_eq!(store.settings().window_width, 1200);
    }

    #[test]
    fn writes_no_file_when_nothing_changes() {
        let fixture = Fixture::new();
        let mut store = fixture.store();

        store
            .edit_settings(|editor| editor.set_window_width(750))
            .expect("edit");
        store.edit_shortcuts(|_| Ok(())).expect("edit");

        assert_eq!(fixture.read(ConfigFile::Settings), None);
        assert_eq!(fixture.read(ConfigFile::Shortcuts), None);
    }

    #[test]
    fn rejects_a_change_with_an_error_and_writes_nothing() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Settings, "window_width = 900\n");
        let mut store = fixture.store();

        let result = store.edit_settings(|editor| editor.set_window_width(20));

        let Err(EditError::Invalid(problems)) = result else {
            panic!("not rejected: {result:?}");
        };
        assert_eq!(
            problems[0].kind,
            ProblemKind::WindowWidthOutOfRange { width: 20 }
        );
        assert_eq!(
            fixture.read(ConfigFile::Settings).as_deref(),
            Some("window_width = 900\n")
        );
        assert_eq!(store.settings().window_width, 900);
    }

    #[test]
    fn does_not_change_a_file_with_a_toml_error() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Settings, "window_width = \n");
        let mut store = fixture.store();

        let result = store.edit_settings(|editor| editor.set_window_width(800));

        assert!(matches!(
            result,
            Err(EditError::File(ConfigError::Toml { .. }))
        ));
        assert_eq!(
            fixture.read(ConfigFile::Settings).as_deref(),
            Some("window_width = \n")
        );
    }

    #[test]
    fn adds_a_shortcut_and_makes_the_file() {
        let fixture = Fixture::new();
        let mut store = fixture.store();

        store
            .edit_shortcuts(|editor| {
                editor.add(&map_shortcut());
                Ok(())
            })
            .expect("edit");

        assert_eq!(store.shortcuts(), &[map_shortcut()]);
        let text = fixture.read(ConfigFile::Shortcuts).expect("new file");
        assert!(text.starts_with("# Shortcuts of petlauncher."), "{text}");
        assert_eq!(store.reload(ConfigFile::Shortcuts), Reload::Unchanged);
    }

    #[test]
    fn does_not_change_shortcuts_by_number_after_a_change_on_the_disk() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Shortcuts, SHORTCUTS);
        let mut store = fixture.store();
        let changed_on_disk = format!("{SHORTCUTS}\n{}", SHORTCUTS.replace("\"w\"", "\"w2\""));
        fixture.write(ConfigFile::Shortcuts, &changed_on_disk);

        let result = store.edit_shortcuts(|editor| editor.remove(0));

        assert_eq!(result, Err(EditError::FileChanged));
        assert_eq!(fixture.read(ConfigFile::Shortcuts), Some(changed_on_disk));
        assert_eq!(store.shortcuts().len(), 2);
    }

    #[test]
    fn gives_the_error_of_the_change_and_writes_nothing() {
        let fixture = Fixture::new();
        fixture.write(ConfigFile::Shortcuts, SHORTCUTS);
        let mut store = fixture.store();

        let result = store.edit_shortcuts(|editor| {
            editor.add(&map_shortcut());
            editor.remove(7)
        });

        assert_eq!(result, Err(EditError::NoShortcut { index: 7, count: 2 }));
        assert_eq!(
            fixture.read(ConfigFile::Shortcuts).as_deref(),
            Some(SHORTCUTS)
        );
        assert_eq!(store.shortcuts().len(), 1);
    }

    #[test]
    fn rejects_a_shortcut_with_an_empty_keyword() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let no_keyword = Shortcut {
            keyword: String::new(),
            ..map_shortcut()
        };

        let result = store.edit_shortcuts(|editor| {
            editor.add(&no_keyword);
            Ok(())
        });

        assert!(matches!(result, Err(EditError::Invalid(_))), "{result:?}");
        assert_eq!(fixture.read(ConfigFile::Shortcuts), None);
    }

    #[test]
    fn gives_the_conflicts_between_the_files_with_the_line() {
        let fixture = Fixture::new();
        fixture.write(
            ConfigFile::Shortcuts,
            &SHORTCUTS.replace("keyword = \"w\"", "keyword = \"d\""),
        );
        fixture.write(ConfigFile::Settings, "windw_width = 1\n");

        let store = fixture.store();

        assert_eq!(
            store.warnings(),
            vec![
                Problem {
                    file: ConfigFile::Settings,
                    line: Some(1),
                    kind: ProblemKind::UnknownKey {
                        key: "windw_width".to_owned()
                    },
                },
                Problem {
                    file: ConfigFile::Shortcuts,
                    line: Some(3),
                    kind: ProblemKind::KeywordOfBuiltIn {
                        keyword: "d".to_owned(),
                        shortcut: ShortcutRef {
                            index: 0,
                            name: "Wiki".to_owned()
                        },
                        built_in: BuiltIn::Dictionary,
                    },
                },
            ]
        );
    }
}
