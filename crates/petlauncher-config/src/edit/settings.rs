use std::path::Path;

use toml_edit::{DocumentMut, InlineTable, Item, Table, TableLike, Value};

use super::format::{multi_line_array, push_entry, remove_entry, set_value};
use crate::error::ConfigError;
use crate::load::deserialize;
use crate::model::{CustomPosition, KeepQuery, Settings, WindowPosition};

/// The text of a new `settings.toml`, with a comment for each key.
pub(crate) const SETTINGS_TEMPLATE: &str = include_str!("../templates/settings.toml");

/// Changes the text of `settings.toml`. Each setter changes only its own
/// value. The comments, the spaces and the order of the other items stay.
/// A setter that gives the value that the file already has changes
/// nothing.
#[derive(Debug, Clone)]
pub struct SettingsEditor {
    document: DocumentMut,
    current: Settings,
}

impl SettingsEditor {
    /// An editor for a new file, from the template.
    pub(crate) fn new_file() -> Self {
        Self::parse(SETTINGS_TEMPLATE, Path::new("settings.toml"))
            .expect("the template is a valid settings file")
    }

    /// An editor for the text of an existing file. A TOML error or a
    /// value with the wrong type gives an error, because the editor must
    /// know the values in the file.
    pub(crate) fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let (document, current) = deserialize::<Settings>(text, path)?;
        Ok(Self {
            document: document.into_mut(),
            current,
        })
    }

    /// The values after the changes.
    pub fn settings(&self) -> &Settings {
        &self.current
    }

    /// The text after the changes.
    pub fn text(&self) -> String {
        self.document.to_string()
    }

    pub fn set_close_on_focus_loss(&mut self, value: bool) {
        if self.current.close_on_focus_loss != value {
            self.current.close_on_focus_loss = value;
            self.set(None, "close_on_focus_loss", value);
        }
    }

    pub fn set_run_in_background(&mut self, value: bool) {
        if self.current.run_in_background != value {
            self.current.run_in_background = value;
            self.set(None, "run_in_background", value);
        }
    }

    pub fn set_show_tray_icon(&mut self, value: bool) {
        if self.current.show_tray_icon != value {
            self.current.show_tray_icon = value;
            self.set(None, "show_tray_icon", value);
        }
    }

    pub fn set_include_applications(&mut self, value: bool) {
        if self.current.include_applications != value {
            self.current.include_applications = value;
            self.set(None, "include_applications", value);
        }
    }

    pub fn set_hotkey(&mut self, value: &str) {
        if self.current.hotkey != value {
            self.current.hotkey = value.to_owned();
            self.set(None, "hotkey", value);
        }
    }

    pub fn set_window_width(&mut self, value: u32) {
        if self.current.window_width != value {
            self.current.window_width = value;
            self.set(None, "window_width", i64::from(value));
        }
    }

    pub fn set_window_position(&mut self, value: WindowPosition) {
        if self.current.window_position != value {
            self.current.window_position = value;
            self.set(None, "window_position", value.name());
        }
    }

    /// The place that the owner pinned: `custom_position` with 4 decimals,
    /// and `window_position = "custom"`.
    pub fn pin_position(&mut self, place: CustomPosition) {
        let round = |value: f64| (value * 10_000.0).round() / 10_000.0;
        let place = CustomPosition {
            x: round(place.x),
            y: round(place.y),
        };
        if self.current.custom_position != Some(place) {
            self.current.custom_position = Some(place);
            let mut table = InlineTable::new();
            table.insert("x", place.x.into());
            table.insert("y", place.y.into());
            self.set(None, "custom_position", table);
        }
        self.set_window_position(WindowPosition::Custom);
    }

    pub fn set_keep_query(&mut self, value: KeepQuery) {
        if self.current.keep_query != value {
            self.current.keep_query = value;
            self.set(None, "keep_query", value.to_string());
        }
    }

    pub fn set_file_search_keyword(&mut self, value: &str) {
        if self.current.file_search.keyword != value {
            self.current.file_search.keyword = value.to_owned();
            self.set(Some("file_search"), "keyword", value);
        }
    }

    /// Adds a directory to `file_search.directories`. `false` when the
    /// list has it already.
    pub fn add_file_search_directory(&mut self, directory: &str) -> bool {
        self.add_to_list(List::Directories, directory)
    }

    /// Removes each entry that is equal to `directory`. `false` when the
    /// list does not have it.
    pub fn remove_file_search_directory(&mut self, directory: &str) -> bool {
        self.remove_from_list(List::Directories, directory)
    }

    /// Adds an entry to `file_search.ignore`. `false` when the list has it
    /// already.
    pub fn add_ignore(&mut self, entry: &str) -> bool {
        self.add_to_list(List::Ignore, entry)
    }

    /// Removes each entry that is equal to `entry`. `false` when the list
    /// does not have it.
    pub fn remove_ignore(&mut self, entry: &str) -> bool {
        self.remove_from_list(List::Ignore, entry)
    }

    pub fn set_dictionary_keyword(&mut self, value: &str) {
        if self.current.dictionary.keyword != value {
            self.current.dictionary.keyword = value.to_owned();
            self.set(Some("dictionary"), "keyword", value);
        }
    }

    pub fn set_wordnet_path(&mut self, value: &str) {
        if self.current.dictionary.wordnet_path != value {
            self.current.dictionary.wordnet_path = value.to_owned();
            self.set(Some("dictionary"), "wordnet_path", value);
        }
    }

    fn set(&mut self, table: Option<&str>, key: &str, value: impl Into<Value>) {
        set_value(self.table_mut(table), key, value);
    }

    /// The root table, or the named table. A missing table is added at the
    /// end of the file.
    fn table_mut(&mut self, name: Option<&str>) -> &mut dyn TableLike {
        let Some(name) = name else {
            return self.document.as_table_mut();
        };
        self.document
            .entry(name)
            .or_insert(Item::Table(Table::new()))
            .as_table_like_mut()
            .expect("the file gave Settings, so this item is a table")
    }

    fn list_mut(&mut self, list: List) -> &mut Vec<String> {
        match list {
            List::Directories => &mut self.current.file_search.directories,
            List::Ignore => &mut self.current.file_search.ignore,
        }
    }

    fn add_to_list(&mut self, list: List, entry: &str) -> bool {
        let entries = self.list_mut(list);
        if entries.iter().any(|existing| existing == entry) {
            return false;
        }
        entries.push(entry.to_owned());
        let entries = entries.clone();
        let table = self.table_mut(Some("file_search"));
        match table.get_mut(list.key()).and_then(Item::as_array_mut) {
            Some(array) => push_entry(array, entry),
            None => set_value(table, list.key(), multi_line_array(&entries)),
        }
        true
    }

    fn remove_from_list(&mut self, list: List, entry: &str) -> bool {
        let entries = self.list_mut(list);
        if !entries.iter().any(|existing| existing == entry) {
            return false;
        }
        entries.retain(|existing| existing != entry);
        let entries = entries.clone();
        let table = self.table_mut(Some("file_search"));
        match table.get_mut(list.key()).and_then(Item::as_array_mut) {
            Some(array) => loop {
                let Some(index) = array.iter().position(|value| value.as_str() == Some(entry))
                else {
                    break;
                };
                remove_entry(array, index);
            },
            None => set_value(table, list.key(), multi_line_array(&entries)),
        }
        true
    }
}

#[derive(Debug, Clone, Copy)]
enum List {
    Directories,
    Ignore,
}

impl List {
    fn key(self) -> &'static str {
        match self {
            Self::Directories => "directories",
            Self::Ignore => "ignore",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::parse_settings;

    const OWNER_TEXT: &str = r#"# My petlauncher settings.

window_width = 750 # wide enough

[file_search]
keyword = "f"
ignore = [
    "node_modules",
    "~/tmp", # my TMPDIR
]
"#;

    fn owner_editor() -> SettingsEditor {
        SettingsEditor::parse(OWNER_TEXT, Path::new("settings.toml")).expect("valid text")
    }

    #[test]
    fn changes_only_the_changed_value() {
        let mut editor = owner_editor();

        editor.set_window_width(900);

        assert_eq!(editor.text(), OWNER_TEXT.replace("750", "900"));
        assert_eq!(editor.settings().window_width, 900);
    }

    #[test]
    fn changes_nothing_when_the_value_does_not_change() {
        let mut editor = owner_editor();

        editor.set_window_width(750);
        editor.set_close_on_focus_loss(true);
        editor.set_file_search_keyword("f");

        assert_eq!(editor.text(), OWNER_TEXT);
    }

    #[test]
    fn adds_a_key_that_the_file_does_not_have_after_the_other_keys() {
        let mut editor = owner_editor();

        editor.set_hotkey("<Super>x");

        assert_eq!(
            editor.text(),
            OWNER_TEXT.replace(
                "750 # wide enough\n",
                "750 # wide enough\nhotkey = \"<Super>x\"\n"
            )
        );
    }

    #[test]
    fn adds_a_table_that_the_file_does_not_have_at_the_end() {
        let mut editor = owner_editor();

        editor.set_wordnet_path("/opt/wordnet");

        assert_eq!(
            editor.text(),
            format!("{OWNER_TEXT}\n[dictionary]\nwordnet_path = \"/opt/wordnet\"\n")
        );
    }

    #[test]
    fn adds_an_ignore_path_and_keeps_the_comments() {
        let mut editor = owner_editor();

        let added = editor.add_ignore("~/lolcat-evidence");

        assert!(added);
        assert_eq!(
            editor.text(),
            OWNER_TEXT.replace("# my TMPDIR\n", "# my TMPDIR\n    \"~/lolcat-evidence\",\n")
        );
    }

    #[test]
    fn does_not_add_an_entry_that_the_list_has() {
        let mut editor = owner_editor();

        assert!(!editor.add_ignore("~/tmp"));
        assert!(!editor.remove_ignore("~/never"));
        assert_eq!(editor.text(), OWNER_TEXT);
    }

    #[test]
    fn removes_an_ignore_entry_and_keeps_the_comment_of_the_other_entry() {
        let mut editor = owner_editor();

        let removed = editor.remove_ignore("node_modules");

        assert!(removed);
        assert_eq!(
            editor.text(),
            OWNER_TEXT.replace("    \"node_modules\",\n", "")
        );
        assert_eq!(editor.settings().file_search.ignore, vec!["~/tmp"]);
    }

    #[test]
    fn changes_the_initial_list_when_the_file_has_no_list() {
        let mut editor =
            SettingsEditor::parse("window_width = 750\n", Path::new("s")).expect("valid text");

        editor.add_file_search_directory("~/work");
        editor.remove_ignore("target");

        let parsed = parse_settings(&editor.text(), Path::new("s")).expect("valid result");
        let mut expected = Settings::default();
        expected.file_search.directories.push("~/work".to_owned());
        expected
            .file_search
            .ignore
            .retain(|entry| entry != "target");
        assert_eq!(parsed.value, expected);
        assert_eq!(editor.settings(), &expected);
    }

    #[test]
    fn writes_each_setting_so_that_the_file_gives_it_back() {
        let mut editor = SettingsEditor::new_file();

        editor.set_close_on_focus_loss(false);
        editor.set_run_in_background(false);
        editor.set_show_tray_icon(false);
        editor.set_include_applications(false);
        editor.set_hotkey("<Super>l");
        editor.set_window_width(1000);
        editor.set_keep_query(KeepQuery::Forever);
        editor.set_file_search_keyword("ff");
        editor.add_file_search_directory("/data");
        editor.remove_file_search_directory("~");
        editor.add_ignore("build");
        editor.set_dictionary_keyword("wn");
        editor.set_wordnet_path("/opt/wordnet");

        let parsed = parse_settings(&editor.text(), Path::new("s")).expect("valid result");
        let mut expected = Settings {
            close_on_focus_loss: false,
            run_in_background: false,
            show_tray_icon: false,
            include_applications: false,
            hotkey: "<Super>l".to_owned(),
            window_width: 1000,
            keep_query: KeepQuery::Forever,
            ..Settings::default()
        };
        expected.file_search.keyword = "ff".to_owned();
        expected.file_search.directories = vec!["/data".to_owned()];
        expected.file_search.ignore.push("build".to_owned());
        expected.dictionary.keyword = "wn".to_owned();
        expected.dictionary.wordnet_path = "/opt/wordnet".to_owned();
        assert_eq!(parsed.value, expected);
        assert_eq!(parsed.warnings, vec![]);
        assert_eq!(editor.settings(), &expected);
    }

    #[test]
    fn writes_the_keep_query_time_as_text() {
        let mut editor = SettingsEditor::new_file();

        editor.set_keep_query(KeepQuery::Seconds(300));

        assert_eq!(
            editor.text(),
            SETTINGS_TEMPLATE.replace("keep_query = \"30s\"", "keep_query = \"5m\"")
        );
    }

    #[test]
    fn starts_a_new_file_from_the_template_with_the_initial_values() {
        let editor = SettingsEditor::new_file();

        let parsed = parse_settings(&editor.text(), Path::new("s")).expect("valid template");

        assert_eq!(editor.text(), SETTINGS_TEMPLATE);
        assert_eq!(parsed.value, Settings::default());
        assert_eq!(parsed.warnings, vec![]);
    }

    #[test]
    fn does_not_edit_a_file_with_a_toml_error() {
        let result = SettingsEditor::parse("window_width = \"wide\"\n", Path::new("s"));

        assert!(matches!(result, Err(ConfigError::Toml { .. })));
    }

    #[test]
    fn edits_a_file_with_a_value_that_validation_rejects() {
        let mut editor =
            SettingsEditor::parse("window_width = 10\n", Path::new("s")).expect("valid TOML");

        editor.set_window_width(800);

        assert_eq!(editor.text(), "window_width = 800\n");
    }
}
