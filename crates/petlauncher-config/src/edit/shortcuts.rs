use std::path::Path;

use toml_edit::{ArrayOfTables, DocumentMut, InlineTable, Item, Table, Value};

use super::format::{has_comment, set_value, split_prefix};
use crate::error::{ConfigError, EditError};
use crate::load::{deserialize, ShortcutsFile};
use crate::model::{Browser, Shortcut};

/// The text of a new `shortcuts.toml`, with a comment for each key.
pub(crate) const SHORTCUTS_TEMPLATE: &str = include_str!("../templates/shortcuts.toml");

/// Changes the text of `shortcuts.toml`. A shortcut is identified by its
/// position in the file, from 0.
///
/// The comment lines directly above a `[[shortcut]]` header belong to that
/// shortcut: they go with it when it moves, and away with it when it is
/// removed. The lines up to and including the last empty line above the
/// header belong to the place in the file and stay there, so the comment
/// at the start of the file stays at the start.
///
/// A file that has the shortcuts as an inline list, `shortcut = [{ ... }]`,
/// gets one `[[shortcut]]` block for each shortcut at its first change.
#[derive(Debug, Clone)]
pub struct ShortcutsEditor {
    document: DocumentMut,
    current: Vec<Shortcut>,
}

impl ShortcutsEditor {
    /// An editor for a new file, from the template.
    pub(crate) fn new_file() -> Self {
        Self::parse(SHORTCUTS_TEMPLATE, Path::new("shortcuts.toml"))
            .expect("the template is a valid shortcuts file")
    }

    /// An editor for the text of an existing file. A TOML error or a
    /// value with the wrong type gives an error.
    pub(crate) fn parse(text: &str, path: &Path) -> Result<Self, ConfigError> {
        let (document, file) = deserialize::<ShortcutsFile>(text, path)?;
        let mut document = document.into_mut();
        if let Some(item) = document.get_mut("shortcut") {
            if !item.is_array_of_tables() {
                let inline = std::mem::take(item);
                *item = inline
                    .into_array_of_tables()
                    .map_or_else(|same| same, Item::ArrayOfTables);
            }
        }
        Ok(Self {
            document,
            current: file.shortcut,
        })
    }

    /// The shortcuts after the changes.
    pub fn shortcuts(&self) -> &[Shortcut] {
        &self.current
    }

    /// The text after the changes.
    pub fn text(&self) -> String {
        self.document.to_string()
    }

    /// Adds a shortcut at the end of the file. The block has `name`,
    /// `keyword` and `cmd`, and the other keys only when they do not have
    /// their initial value.
    pub fn add(&mut self, shortcut: &Shortcut) {
        let mut table = new_table(shortcut);
        let prefix = if self.current.is_empty() {
            // A file with no tables keeps all its comments after the last
            // item. They must stay above the first shortcut.
            let comments = self
                .document
                .trailing()
                .as_str()
                .unwrap_or_default()
                .to_owned();
            self.document.set_trailing("");
            if comments.trim().is_empty() {
                String::new()
            } else {
                format!("{}\n\n", comments.trim_end())
            }
        } else {
            "\n".to_owned()
        };
        table.decor_mut().set_prefix(prefix);
        match self
            .document
            .get_mut("shortcut")
            .and_then(Item::as_array_of_tables_mut)
        {
            Some(tables) => tables.push(table),
            None => {
                let mut tables = ArrayOfTables::new();
                tables.push(table);
                self.document
                    .insert("shortcut", Item::ArrayOfTables(tables));
            }
        }
        self.current.push(shortcut.clone());
    }

    /// Replaces the shortcut at `index`. Only the lines of the changed
    /// keys change.
    pub fn change(&mut self, index: usize, shortcut: &Shortcut) -> Result<(), EditError> {
        let old = self.check_index(index)?.clone();
        let table = self.table_mut(index);
        let texts = [
            ("name", &old.name, &shortcut.name),
            ("keyword", &old.keyword, &shortcut.keyword),
            ("cmd", &old.cmd, &shortcut.cmd),
            (
                "cmd_without_argument",
                &old.cmd_without_argument,
                &shortcut.cmd_without_argument,
            ),
            ("icon", &old.icon, &shortcut.icon),
        ];
        for (key, old, new) in texts {
            if old != new {
                set_value(table, key, new.as_str());
            }
        }
        if old.browser != shortcut.browser {
            match &shortcut.browser {
                Some(browser) => set_value(table, "browser", browser_value(browser)),
                None => {
                    table.remove("browser");
                }
            }
        }
        let flags = [
            (
                "is_default_search",
                old.is_default_search,
                shortcut.is_default_search,
            ),
            (
                "run_without_argument",
                old.run_without_argument,
                shortcut.run_without_argument,
            ),
        ];
        for (key, old, new) in flags {
            if old != new {
                set_value(table, key, new);
            }
        }
        self.current[index] = shortcut.clone();
        Ok(())
    }

    pub fn remove(&mut self, index: usize) -> Result<(), EditError> {
        self.check_index(index)?;
        let tables = self.tables_mut();
        let removed = tables.remove(index);
        let (removed_place, _) = split_prefix(prefix_of(&removed));
        let removed_place = removed_place.to_owned();
        let mut place_without_table = None;
        match tables.get_mut(index) {
            Some(next) => {
                let next_prefix = prefix_of(next).to_owned();
                let (next_place, next_own) = split_prefix(&next_prefix);
                next.decor_mut()
                    .set_prefix(format!("{}{next_own}", merge(&removed_place, next_place)));
            }
            None if has_comment(&removed_place) => place_without_table = Some(removed_place),
            None => {}
        }
        if tables.is_empty() {
            self.document.remove("shortcut");
        }
        if let Some(place) = place_without_table {
            let trailing = self.document.trailing().as_str().unwrap_or_default();
            let trailing = format!("{place}{trailing}");
            self.document.set_trailing(trailing);
        }
        self.current.remove(index);
        Ok(())
    }

    /// Moves the shortcut at `from` so that it is at `to` after the move.
    pub fn move_shortcut(&mut self, from: usize, to: usize) -> Result<(), EditError> {
        self.check_index(from)?;
        self.check_index(to)?;
        if from == to {
            return Ok(());
        }
        let mut order: Vec<usize> = (0..self.current.len()).collect();
        let moved = order.remove(from);
        order.insert(to, moved);

        let tables: Vec<Table> = self.tables_mut().iter().cloned().collect();
        let places: Vec<(String, Option<isize>)> = tables
            .iter()
            .map(|table| {
                let (place, _) = split_prefix(prefix_of(table));
                (place.to_owned(), table.position())
            })
            .collect();
        let mut reordered = ArrayOfTables::new();
        for ((place, position), &source) in places.into_iter().zip(&order) {
            let mut table = tables[source].clone();
            let (_, own) = split_prefix(prefix_of(&table));
            let prefix = format!("{place}{own}");
            table.decor_mut().set_prefix(prefix);
            table.set_position(position);
            reordered.push(table);
        }
        *self.tables_mut() = reordered;
        self.current = order
            .iter()
            .map(|&index| self.current[index].clone())
            .collect();
        Ok(())
    }

    fn check_index(&self, index: usize) -> Result<&Shortcut, EditError> {
        self.current.get(index).ok_or(EditError::NoShortcut {
            index,
            count: self.current.len(),
        })
    }

    fn tables_mut(&mut self) -> &mut ArrayOfTables {
        self.document
            .get_mut("shortcut")
            .and_then(Item::as_array_of_tables_mut)
            .expect("the file has shortcuts, so it has the array of tables")
    }

    fn table_mut(&mut self, index: usize) -> &mut Table {
        self.tables_mut()
            .get_mut(index)
            .expect("the index was checked")
    }
}

fn new_table(shortcut: &Shortcut) -> Table {
    let mut table = Table::new();
    table.insert("name", toml_edit::value(&shortcut.name));
    table.insert("keyword", toml_edit::value(&shortcut.keyword));
    table.insert("cmd", toml_edit::value(&shortcut.cmd));
    for (key, text) in [
        ("cmd_without_argument", &shortcut.cmd_without_argument),
        ("icon", &shortcut.icon),
    ] {
        if !text.is_empty() {
            table.insert(key, toml_edit::value(text));
        }
    }
    if let Some(browser) = &shortcut.browser {
        table.insert("browser", Item::Value(browser_value(browser)));
    }
    for (key, flag) in [
        ("is_default_search", shortcut.is_default_search),
        ("run_without_argument", shortcut.run_without_argument),
    ] {
        if flag {
            table.insert(key, toml_edit::value(true));
        }
    }
    table
}

fn browser_value(browser: &Browser) -> Value {
    let (key, text) = match browser {
        Browser::App(app) => ("app", app),
        Browser::Command(command) => ("command", command),
    };
    let mut table = InlineTable::new();
    table.insert(key, Value::from(text.as_str()));
    Value::InlineTable(table)
}

fn prefix_of(table: &Table) -> &str {
    table
        .decor()
        .prefix()
        .and_then(|prefix| prefix.as_str())
        .unwrap_or_default()
}

/// Joins the text of two places in the file into one. Text with no comment
/// is only spaces and empty lines, and the other text replaces it.
fn merge(first: &str, second: &str) -> String {
    match (has_comment(first), has_comment(second)) {
        (_, false) => first.to_owned(),
        (false, true) => second.to_owned(),
        (true, true) => format!("{first}{second}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load::parse_shortcuts;

    const OWNER_TEXT: &str = r#"# My shortcuts.

# Search the web.
[[shortcut]]
name = "Search"
keyword = "g"
cmd = "https://search.example.com/?q=%s" # the search engine
is_default_search = true

[[shortcut]]
name = "Wiki"
keyword = "w"
cmd = "https://wiki.example.com/%s"

# Work
[[shortcut]]
name = "Tickets"
keyword = "t"
cmd = "https://tickets.example.com/browse/%s"
# end of file
"#;

    const SEARCH: &str = r#"# Search the web.
[[shortcut]]
name = "Search"
keyword = "g"
cmd = "https://search.example.com/?q=%s" # the search engine
is_default_search = true
"#;

    const WIKI: &str = r#"[[shortcut]]
name = "Wiki"
keyword = "w"
cmd = "https://wiki.example.com/%s"
"#;

    const TICKETS: &str = r#"# Work
[[shortcut]]
name = "Tickets"
keyword = "t"
cmd = "https://tickets.example.com/browse/%s"
"#;

    fn owner_editor() -> ShortcutsEditor {
        ShortcutsEditor::parse(OWNER_TEXT, Path::new("shortcuts.toml")).expect("valid text")
    }

    fn shortcut(name: &str, keyword: &str, cmd: &str) -> Shortcut {
        Shortcut {
            name: name.to_owned(),
            keyword: keyword.to_owned(),
            cmd: cmd.to_owned(),
            ..Shortcut::default()
        }
    }

    fn reparsed(editor: &ShortcutsEditor) -> Vec<Shortcut> {
        let loaded = parse_shortcuts(&editor.text(), Path::new("s")).expect("valid result");
        assert_eq!(loaded.value, editor.shortcuts());
        loaded.value
    }

    #[test]
    fn adds_the_first_shortcut_after_the_comment_of_the_template() {
        let mut editor = ShortcutsEditor::new_file();
        let new = shortcut("Map", "m", "https://maps.example.com/?q=%s");

        editor.add(&new);

        assert_eq!(
            editor.text(),
            format!(
                "{SHORTCUTS_TEMPLATE}\n[[shortcut]]\nname = \"Map\"\nkeyword = \"m\"\ncmd = \"https://maps.example.com/?q=%s\"\n"
            )
        );
        assert_eq!(reparsed(&editor), vec![new]);
    }

    #[test]
    fn adds_a_shortcut_after_the_last_shortcut() {
        let mut editor = owner_editor();

        editor.add(&shortcut("Map", "m", "https://maps.example.com/?q=%s"));

        assert_eq!(
            editor.text(),
            OWNER_TEXT.replace(
                "# end of file\n",
                "\n[[shortcut]]\nname = \"Map\"\nkeyword = \"m\"\ncmd = \"https://maps.example.com/?q=%s\"\n# end of file\n"
            )
        );
        assert_eq!(reparsed(&editor).len(), 4);
    }

    #[test]
    fn writes_a_script_as_a_multi_line_string_and_only_the_keys_that_are_set() {
        let mut editor = ShortcutsEditor::new_file();
        let script = Shortcut {
            name: "Review".to_owned(),
            keyword: "rv".to_owned(),
            cmd: "#!/bin/sh\nexec example-browser \"https://review.example.com/$1\"\n".to_owned(),
            browser: Some(Browser::Command(
                "example-browser --profile work".to_owned(),
            )),
            run_without_argument: true,
            ..Shortcut::default()
        };

        editor.add(&script);

        assert!(
            editor.text().ends_with(
                "[[shortcut]]
name = \"Review\"
keyword = \"rv\"
cmd = \"\"\"
#!/bin/sh
exec example-browser \"https://review.example.com/$1\"
\"\"\"
browser = { command = \"example-browser --profile work\" }
run_without_argument = true
"
            ),
            "{}",
            editor.text()
        );
        assert_eq!(reparsed(&editor), vec![script]);
    }

    #[test]
    fn changes_only_the_lines_of_the_changed_keys() {
        let mut editor = owner_editor();
        let mut search = editor.shortcuts()[0].clone();
        search.keyword = "s".to_owned();
        search.cmd = "https://other.example.com/?q=%s".to_owned();
        search.icon = "web-browser".to_owned();

        editor.change(0, &search).expect("change");

        assert_eq!(
            editor.text(),
            OWNER_TEXT
                .replace("keyword = \"g\"", "keyword = \"s\"")
                .replace("search.example.com", "other.example.com")
                .replace(
                    "is_default_search = true\n",
                    "is_default_search = true\nicon = \"web-browser\"\n"
                )
        );
        assert_eq!(reparsed(&editor)[0], search);
    }

    #[test]
    fn adds_and_removes_the_browser_of_a_shortcut() {
        let mut editor = owner_editor();
        let mut wiki = editor.shortcuts()[1].clone();
        wiki.browser = Some(Browser::App("example-browser.desktop".to_owned()));

        editor.change(1, &wiki).expect("add browser");

        assert_eq!(
            editor.text(),
            OWNER_TEXT.replace(
                "wiki.example.com/%s\"\n",
                "wiki.example.com/%s\"\nbrowser = { app = \"example-browser.desktop\" }\n"
            )
        );
        wiki.browser = None;
        editor.change(1, &wiki).expect("remove browser");
        assert_eq!(editor.text(), OWNER_TEXT);
    }

    #[test]
    fn changes_nothing_when_the_shortcut_is_the_same() {
        let mut editor = owner_editor();
        let same = editor.shortcuts()[2].clone();

        editor.change(2, &same).expect("change");

        assert_eq!(editor.text(), OWNER_TEXT);
    }

    #[test]
    fn removes_the_first_shortcut_and_keeps_the_comment_at_the_start_of_the_file() {
        let mut editor = owner_editor();

        editor.remove(0).expect("remove");

        assert_eq!(
            editor.text(),
            format!("# My shortcuts.\n\n{WIKI}\n{TICKETS}# end of file\n")
        );
        assert_eq!(reparsed(&editor).len(), 2);
    }

    #[test]
    fn removes_a_shortcut_in_the_middle_and_keeps_the_comment_of_the_next() {
        let mut editor = owner_editor();

        editor.remove(1).expect("remove");

        assert_eq!(
            editor.text(),
            format!("# My shortcuts.\n\n{SEARCH}\n{TICKETS}# end of file\n")
        );
    }

    #[test]
    fn removes_the_last_shortcut_with_its_comment() {
        let mut editor = owner_editor();

        editor.remove(2).expect("remove");

        assert_eq!(
            editor.text(),
            format!("# My shortcuts.\n\n{SEARCH}\n{WIKI}# end of file\n")
        );
    }

    #[test]
    fn keeps_the_comments_of_the_file_when_it_removes_all_shortcuts() {
        let mut editor = owner_editor();

        for _ in 0..3 {
            editor.remove(0).expect("remove");
        }

        assert_eq!(editor.text(), "# My shortcuts.\n\n# end of file\n");
        assert_eq!(reparsed(&editor), vec![]);
    }

    #[test]
    fn moves_a_shortcut_with_the_comment_above_it() {
        let mut editor = owner_editor();

        editor.move_shortcut(2, 0).expect("move");

        assert_eq!(
            editor.text(),
            format!("# My shortcuts.\n\n{TICKETS}\n{SEARCH}\n{WIKI}# end of file\n")
        );
        let keywords: Vec<String> = reparsed(&editor)
            .into_iter()
            .map(|shortcut| shortcut.keyword)
            .collect();
        assert_eq!(keywords, ["t", "g", "w"]);
    }

    #[test]
    fn moves_a_shortcut_down() {
        let mut editor = owner_editor();

        editor.move_shortcut(0, 1).expect("move");

        assert_eq!(
            editor.text(),
            format!("# My shortcuts.\n\n{WIKI}\n{SEARCH}\n{TICKETS}# end of file\n")
        );
    }

    #[test]
    fn gives_an_error_for_a_shortcut_that_does_not_exist() {
        let mut editor = owner_editor();
        let any = shortcut("A", "a", "x");

        assert_eq!(
            editor.change(3, &any),
            Err(EditError::NoShortcut { index: 3, count: 3 })
        );
        assert_eq!(
            editor.remove(5),
            Err(EditError::NoShortcut { index: 5, count: 3 })
        );
        assert_eq!(
            editor.move_shortcut(0, 3),
            Err(EditError::NoShortcut { index: 3, count: 3 })
        );
        assert_eq!(editor.move_shortcut(1, 1), Ok(()));
        assert_eq!(editor.text(), OWNER_TEXT);
    }

    #[test]
    fn changes_shortcuts_that_the_file_has_as_an_inline_list() {
        let text = "shortcut = [\n  { name = \"A\", keyword = \"a\", cmd = \"x\" },\n  { name = \"B\", keyword = \"b\", cmd = \"y\" },\n]\n";
        let mut editor = ShortcutsEditor::parse(text, Path::new("s")).expect("valid text");

        editor.add(&shortcut("C", "c", "z"));
        editor.remove(0).expect("remove");

        let keywords: Vec<String> = reparsed(&editor)
            .into_iter()
            .map(|shortcut| shortcut.keyword)
            .collect();
        assert_eq!(keywords, ["b", "c"]);
    }

    #[test]
    fn starts_a_new_file_from_the_template_with_no_shortcuts() {
        let editor = ShortcutsEditor::new_file();

        let loaded = parse_shortcuts(&editor.text(), Path::new("s")).expect("valid template");

        assert_eq!(editor.text(), SHORTCUTS_TEMPLATE);
        assert_eq!(loaded.value, vec![]);
        assert_eq!(loaded.warnings, vec![]);
    }
}
