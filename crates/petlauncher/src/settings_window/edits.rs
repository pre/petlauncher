//! The changes that the settings window makes, and their mapping to the
//! editors of the store (R18). Each change writes only its own value, so the
//! comments and the layout of the files stay.

use std::path::Path;

use petlauncher_config::{EditError, KeepQuery, SettingsEditor, Shortcut, ShortcutsEditor};
use petlauncher_index::IgnoreEntry;

/// A change of `settings.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsEdit {
    CloseOnFocusLoss(bool),
    RunInBackground(bool),
    ShowTrayIcon(bool),
    IncludeApplications(bool),
    WindowWidth(u32),
    KeepQuery(KeepQuery),
    FileSearchKeyword(String),
    AddDirectory(String),
    RemoveDirectory(String),
    AddIgnore(String),
    RemoveIgnore(String),
    DictionaryKeyword(String),
    WordnetPath(String),
}

impl SettingsEdit {
    /// Makes the change with the setter of the editor.
    pub fn apply(&self, editor: &mut SettingsEditor) {
        match self {
            Self::CloseOnFocusLoss(value) => editor.set_close_on_focus_loss(*value),
            Self::RunInBackground(value) => editor.set_run_in_background(*value),
            Self::ShowTrayIcon(value) => editor.set_show_tray_icon(*value),
            Self::IncludeApplications(value) => editor.set_include_applications(*value),
            Self::WindowWidth(value) => editor.set_window_width(*value),
            Self::KeepQuery(value) => editor.set_keep_query(*value),
            Self::FileSearchKeyword(value) => editor.set_file_search_keyword(value),
            Self::AddDirectory(value) => {
                editor.add_file_search_directory(value);
            }
            Self::RemoveDirectory(value) => {
                editor.remove_file_search_directory(value);
            }
            Self::AddIgnore(value) => {
                editor.add_ignore(value);
            }
            Self::RemoveIgnore(value) => {
                editor.remove_ignore(value);
            }
            Self::DictionaryKeyword(value) => editor.set_dictionary_keyword(value),
            Self::WordnetPath(value) => editor.set_wordnet_path(value),
        }
    }

    /// The check before the edit, for the values that the store does not
    /// check itself: an ignore entry in its three forms (R21) and a
    /// directory. The error text is for the owner.
    pub fn check(&self, home: &Path) -> Result<(), String> {
        match self {
            Self::AddIgnore(entry) => IgnoreEntry::parse(entry, home)
                .map(|_| ())
                .map_err(|error| capitalize(&error.to_string())),
            Self::AddDirectory(directory) if directory.trim().is_empty() => {
                Err("Type a directory or select one with the folder button".into())
            }
            _ => Ok(()),
        }
    }
}

/// A change of `shortcuts.toml`. A shortcut is identified by its position
/// in the file, from 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShortcutEdit {
    Add(Shortcut),
    Change(usize, Shortcut),
    Remove(usize),
    Move { from: usize, to: usize },
}

impl ShortcutEdit {
    pub fn apply(&self, editor: &mut ShortcutsEditor) -> Result<(), EditError> {
        match self {
            Self::Add(shortcut) => {
                editor.add(shortcut);
                Ok(())
            }
            Self::Change(index, shortcut) => editor.change(*index, shortcut),
            Self::Remove(index) => editor.remove(*index),
            Self::Move { from, to } => editor.move_shortcut(*from, *to),
        }
    }

    /// The move after a drop of the shortcut at `from` on the row `onto`:
    /// above that row, or below it when `lower_half`. `None` when the
    /// shortcut stays at its place.
    pub fn drop(from: usize, onto: usize, lower_half: bool) -> Option<Self> {
        let before = onto + usize::from(lower_half);
        let to = if before > from { before - 1 } else { before };
        (to != from).then_some(Self::Move { from, to })
    }

    /// The move of Alt+Up (`up`) or Alt+Down in a list of `count`
    /// shortcuts. `None` at the end of the list.
    pub fn step(from: usize, up: bool, count: usize) -> Option<Self> {
        let to = if up {
            from.checked_sub(1)?
        } else {
            Some(from + 1).filter(|to| *to < count)?
        };
        Some(Self::Move { from, to })
    }
}

/// Why the window does not change an inline list of shortcuts.
pub const INLINE_SHORTCUTS: &str = "shortcuts.toml has the shortcuts as an inline list, \
     shortcut = [{ ... }]. A change of that list cannot keep its comments, so the window \
     only shows the shortcuts. Write one [[shortcut]] block for each shortcut to change \
     them here.";

/// The text of a refused edit for the owner.
pub fn refused(error: &EditError) -> String {
    match error {
        EditError::File(error) => format!(
            "The file has an error, so petlauncher did not change it. Correct the file first: {error}"
        ),
        EditError::Invalid(_) => format!("The change was not saved: {error}"),
        EditError::FileChanged => "shortcuts.toml changed in an editor. The window shows the new \
             shortcuts now. Do the change again."
            .into(),
        EditError::InlineShortcuts => INLINE_SHORTCUTS.into(),
        EditError::NoShortcut { .. } => capitalize(&error.to_string()),
    }
}

/// A path for the files: `~/...` below the home directory, so the file
/// stays correct for the same owner on a different computer.
pub fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::LauncherFixture;
    use petlauncher_config::Browser;
    use std::fs;

    const SETTINGS: &str = "\
# The settings of petlauncher.
close_on_focus_loss = true # Close at a focus change.

[file_search]
# The keyword of the file search.
keyword = \"f\"
# Ignore these.
ignore = [\"node_modules\"] # names
";

    const SHORTCUTS: &str = "\
# My shortcuts.

# Search the web.
[[shortcut]]
name = \"Search\"
keyword = \"g\"
cmd = \"https://example.com/?q=%s\"

# Pull requests.
[[shortcut]]
name = \"Rektangel\"
keyword = \"rektangel\"
cmd = \"https://github.com/pre/rektangel/pull/%s\"
";

    fn started(fixture: &LauncherFixture) -> crate::backend::Backend {
        fs::write(fixture.settings_file(), SETTINGS).unwrap();
        fs::write(fixture.shortcuts_file(), SHORTCUTS).unwrap();
        fixture.start().0
    }

    #[test]
    fn each_setting_edit_writes_only_its_value_and_keeps_the_comments() {
        let fixture = LauncherFixture::new();
        let backend = started(&fixture);
        let edits = [
            SettingsEdit::CloseOnFocusLoss(false),
            SettingsEdit::WindowWidth(900),
            SettingsEdit::KeepQuery(KeepQuery::Seconds(120)),
            SettingsEdit::AddIgnore("~/lolcat-output".into()),
        ];

        for edit in &edits {
            backend
                .launcher()
                .edit_settings(|editor| edit.apply(editor))
                .unwrap();
        }

        let text = fs::read_to_string(fixture.settings_file()).unwrap();
        assert!(
            text.contains("close_on_focus_loss = false # Close at a focus change."),
            "{text}"
        );
        assert!(
            text.contains("# The keyword of the file search.\nkeyword = \"f\""),
            "{text}"
        );
        assert!(text.contains("\"~/lolcat-output\""), "{text}");
        assert!(text.contains("keep_query = \"2m\""), "{text}");
        let settings = backend.settings();
        assert_eq!(settings.window_width, 900);
        assert!(!settings.close_on_focus_loss);
        assert!(settings
            .file_search
            .ignore
            .contains(&"~/lolcat-output".to_owned()));
    }

    #[test]
    fn the_other_setting_edits_reach_their_setters() {
        let fixture = LauncherFixture::new();
        let backend = started(&fixture);
        let edits = [
            SettingsEdit::RunInBackground(false),
            SettingsEdit::ShowTrayIcon(false),
            SettingsEdit::IncludeApplications(false),
            SettingsEdit::FileSearchKeyword("ff".into()),
            SettingsEdit::AddDirectory("~/projects".into()),
            SettingsEdit::RemoveIgnore("node_modules".into()),
            SettingsEdit::DictionaryKeyword("dd".into()),
            SettingsEdit::WordnetPath("/opt/wordnet".into()),
        ];

        for edit in &edits {
            backend
                .launcher()
                .edit_settings(|editor| edit.apply(editor))
                .unwrap();
        }
        backend
            .launcher()
            .edit_settings(|editor| SettingsEdit::RemoveDirectory("~".into()).apply(editor))
            .unwrap();

        let settings = backend.settings();
        assert!(!settings.run_in_background);
        assert!(!settings.show_tray_icon);
        assert!(!settings.include_applications);
        assert_eq!(settings.file_search.keyword, "ff");
        assert_eq!(settings.file_search.directories, ["~/projects"]);
        assert!(!settings
            .file_search
            .ignore
            .contains(&"node_modules".to_owned()));
        assert_eq!(settings.dictionary.keyword, "dd");
        assert_eq!(settings.dictionary.wordnet_path, "/opt/wordnet");
    }

    #[test]
    fn an_edit_with_an_error_is_not_written() {
        let fixture = LauncherFixture::new();
        let backend = started(&fixture);

        let result = backend
            .launcher()
            .edit_settings(|editor| SettingsEdit::FileSearchKeyword("a b".into()).apply(editor));

        let error = result.unwrap_err();
        assert!(matches!(error, EditError::Invalid(_)));
        assert!(
            refused(&error).starts_with("The change was not saved"),
            "{}",
            refused(&error)
        );
        assert_eq!(
            fs::read_to_string(fixture.settings_file()).unwrap(),
            SETTINGS
        );
    }

    #[test]
    fn a_file_with_an_error_refuses_each_edit_and_stays_unchanged() {
        let fixture = LauncherFixture::new();
        let backend = started(&fixture);
        let broken = format!("{SETTINGS}\nwindow_width = = 3\n");
        fs::write(fixture.settings_file(), &broken).unwrap();

        let error = backend
            .launcher()
            .edit_settings(|editor| SettingsEdit::ShowTrayIcon(false).apply(editor))
            .unwrap_err();

        assert!(matches!(error, EditError::File(_)));
        let text = refused(&error);
        assert!(text.contains("settings.toml:"), "{text}");
        assert!(text.contains("Correct the file first"), "{text}");
        assert_eq!(fs::read_to_string(fixture.settings_file()).unwrap(), broken);
    }

    #[test]
    fn the_ignore_entry_is_checked_in_its_three_forms() {
        let home = Path::new("/home/owner");

        for good in ["~/lolcat-output", "node_modules", "*-wt", "/srv/data/"] {
            assert_eq!(
                SettingsEdit::AddIgnore(good.into()).check(home),
                Ok(()),
                "{good}"
            );
        }
        let bad = SettingsEdit::AddIgnore("[abc".into())
            .check(home)
            .unwrap_err();
        let empty = SettingsEdit::AddIgnore("  ".into())
            .check(home)
            .unwrap_err();

        assert!(
            bad.starts_with("The ignore pattern \"[abc\" is not correct"),
            "{bad}"
        );
        assert_eq!(empty, "An ignore entry is empty");
        assert!(SettingsEdit::AddDirectory(" ".into()).check(home).is_err());
        assert!(SettingsEdit::WindowWidth(10).check(home).is_ok());
    }

    #[test]
    fn shortcut_edits_add_change_move_and_remove_with_their_comments() {
        let fixture = LauncherFixture::new();
        let backend = started(&fixture);
        let mut added = Shortcut {
            name: "Docs".into(),
            keyword: "docs".into(),
            cmd: "https://docs.example.com/%s".into(),
            browser: Some(Browser::App("lolcat-browser.desktop".into())),
            ..Shortcut::default()
        };
        let edits = [
            ShortcutEdit::Add(added.clone()),
            ShortcutEdit::Move { from: 2, to: 0 },
            ShortcutEdit::Remove(1),
        ];

        for edit in &edits {
            backend
                .launcher()
                .edit_shortcuts(|editor| edit.apply(editor))
                .unwrap();
        }
        added.name = "Documents".into();
        backend
            .launcher()
            .edit_shortcuts(|editor| ShortcutEdit::Change(0, added.clone()).apply(editor))
            .unwrap();

        let names: Vec<String> = backend
            .launcher()
            .shortcuts()
            .iter()
            .map(|s| s.name.clone())
            .collect();
        assert_eq!(names, ["Documents", "Rektangel"]);
        let text = fs::read_to_string(fixture.shortcuts_file()).unwrap();
        assert!(text.starts_with("# My shortcuts.\n"), "{text}");
        assert!(text.contains("# Pull requests.\n[[shortcut]]"), "{text}");
        assert!(!text.contains("# Search the web."), "{text}");
    }

    #[test]
    fn a_shortcut_edit_after_a_change_on_the_disk_gives_file_changed_and_the_new_list() {
        let fixture = LauncherFixture::new();
        let backend = started(&fixture);
        fs::write(
            fixture.shortcuts_file(),
            SHORTCUTS.replace("\"Rektangel\"", "\"Rektangel pulls\""),
        )
        .unwrap();

        let error = backend
            .launcher()
            .edit_shortcuts(|editor| ShortcutEdit::Remove(0).apply(editor))
            .unwrap_err();

        assert_eq!(error, EditError::FileChanged);
        assert!(refused(&error).contains("Do the change again"));
        assert_eq!(backend.launcher().shortcuts()[1].name, "Rektangel pulls");
        assert_eq!(backend.launcher().shortcuts().len(), 2);
    }

    #[test]
    fn inline_shortcuts_refuse_each_edit() {
        let fixture = LauncherFixture::new();
        fs::write(
            fixture.shortcuts_file(),
            "shortcut = [{ name = \"Search\", keyword = \"g\", cmd = \"https://example.com/?q=%s\" }]\n",
        )
        .unwrap();
        let backend = fixture.start().0;

        let probe = backend.launcher().edit_shortcuts(|_| Ok(()));
        let remove = backend
            .launcher()
            .edit_shortcuts(|editor| ShortcutEdit::Remove(0).apply(editor));

        assert_eq!(probe, Err(EditError::InlineShortcuts));
        assert_eq!(remove, Err(EditError::InlineShortcuts));
        assert_eq!(refused(&EditError::InlineShortcuts), INLINE_SHORTCUTS);
        assert_eq!(backend.launcher().shortcuts().len(), 1);
    }

    #[test]
    fn a_selected_folder_below_the_home_directory_starts_with_a_tilde() {
        let home = Path::new("/home/owner");

        assert_eq!(
            tilde(Path::new("/home/owner/lolcat-output"), home),
            "~/lolcat-output"
        );
        assert_eq!(tilde(home, home), "~");
        assert_eq!(tilde(Path::new("/srv/data"), home), "/srv/data");
        assert_eq!(tilde(Path::new("/home/owner2/x"), home), "/home/owner2/x");
    }

    #[test]
    fn a_missing_shortcut_is_an_error() {
        let fixture = LauncherFixture::new();
        let backend = started(&fixture);

        let error = backend
            .launcher()
            .edit_shortcuts(|editor| ShortcutEdit::Remove(7).apply(editor))
            .unwrap_err();

        assert!(
            refused(&error).starts_with("There is no shortcut 8"),
            "{}",
            refused(&error)
        );
    }

    fn moved(from: usize, to: usize) -> Option<ShortcutEdit> {
        Some(ShortcutEdit::Move { from, to })
    }

    #[test]
    fn a_drop_on_the_upper_half_of_a_row_puts_the_shortcut_above_it() {
        assert_eq!(ShortcutEdit::drop(3, 1, false), moved(3, 1));
        assert_eq!(ShortcutEdit::drop(1, 3, false), moved(1, 2));
        assert_eq!(ShortcutEdit::drop(4, 0, false), moved(4, 0));
    }

    #[test]
    fn a_drop_on_the_lower_half_of_a_row_puts_the_shortcut_below_it() {
        assert_eq!(ShortcutEdit::drop(3, 1, true), moved(3, 2));
        assert_eq!(ShortcutEdit::drop(1, 3, true), moved(1, 3));
        assert_eq!(ShortcutEdit::drop(0, 4, true), moved(0, 4));
    }

    #[test]
    fn a_drop_at_the_same_place_is_no_edit() {
        assert_eq!(ShortcutEdit::drop(2, 2, false), None);
        assert_eq!(ShortcutEdit::drop(2, 2, true), None);
        assert_eq!(ShortcutEdit::drop(2, 3, false), None);
        assert_eq!(ShortcutEdit::drop(2, 1, true), None);
    }

    #[test]
    fn alt_up_and_alt_down_move_one_place_and_stop_at_the_ends() {
        assert_eq!(ShortcutEdit::step(2, true, 5), moved(2, 1));
        assert_eq!(ShortcutEdit::step(2, false, 5), moved(2, 3));
        assert_eq!(ShortcutEdit::step(0, true, 5), None);
        assert_eq!(ShortcutEdit::step(4, false, 5), None);
    }
}
