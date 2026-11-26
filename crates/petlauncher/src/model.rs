//! The state of the launcher window without widgets: the query text, the
//! rows that show, the selected row, and a key that waits for the results of
//! the text.

use petlauncher_core::{Action, Key, Results};

use crate::selection::Selection;

/// What the program does with new results.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// The results are for an older text. Do not show them.
    Old,
    Show,
    /// Show them, then activate this key, which the owner pressed before
    /// the results came.
    ShowAndActivate(Key),
}

/// What the program does with a key on the rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// The rows are for the query text: call `activate` now.
    Now,
    /// The rows are for an older text. The key waits for the results of the
    /// text now, so that Enter acts on what the owner typed.
    Wait,
}

#[derive(Debug, Default)]
pub struct LauncherModel {
    text: String,
    results: Results,
    selection: Selection,
    pending: Option<Key>,
}

impl LauncherModel {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn results(&self) -> &Results {
        &self.results
    }

    pub fn selected(&self) -> Option<usize> {
        self.selection.selected()
    }

    /// The query text changed. The program searches the new text. A key
    /// that waited for the results of the old text is dropped.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_owned();
        self.pending = None;
    }

    /// An empty query line with no rows, for a show with no kept query.
    /// The core gives no rows for an empty query, so the window can be at
    /// its collapsed height before the search ends.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// A show with the kept query `text` (R9). The rows stay when they are
    /// for this text, so that the window shows with them.
    pub fn restore(&mut self, text: &str) {
        if self.results.query != text {
            self.results = Results::default();
            self.selection = Selection::default();
        }
        self.set_text(text);
    }

    pub fn results_arrived(&mut self, results: Results) -> Arrival {
        if results.query != self.text {
            return Arrival::Old;
        }
        self.selection = Selection::new(results.rows.len(), results.selected);
        self.results = results;
        match self.pending.take() {
            Some(key) => Arrival::ShowAndActivate(key),
            None => Arrival::Show,
        }
    }

    pub fn press(&mut self, key: Key) -> Press {
        if self.results.query == self.text {
            Press::Now
        } else {
            self.pending = Some(key);
            Press::Wait
        }
    }

    pub fn up(&mut self) {
        self.selection.up();
    }

    pub fn down(&mut self) {
        self.selection.down();
    }

    pub fn select(&mut self, index: usize) {
        self.selection.select(index);
    }

    /// The text for Tab: the new query of the selected row, when Enter on it
    /// changes the query. For a directory row of the path mode this is the
    /// path of the directory.
    pub fn completion(&self) -> Option<&str> {
        let row = self.results.rows.get(self.selected()?)?;
        match &row.enter {
            Action::SetQuery(text) if self.results.query == self.text => Some(text),
            _ => None,
        }
    }
}

/// Who executes an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect<'a> {
    Nothing,
    /// The launcher window puts the text in the query line.
    SetQuery(&'a str),
    /// The launcher window puts the text on the clipboard.
    Copy(&'a str),
    /// The dictionary window (R19).
    Dictionary(&'a str),
    /// The desktop: `Backend::execute`.
    Desktop,
}

pub fn effect(action: &Action) -> Effect<'_> {
    match action {
        Action::Nothing => Effect::Nothing,
        Action::SetQuery(text) => Effect::SetQuery(text),
        Action::Copy(text) => Effect::Copy(text),
        Action::OpenDictionary(word) => Effect::Dictionary(word),
        Action::OpenFile(_)
        | Action::OpenFolder(_)
        | Action::ShowInFolder(_)
        | Action::LaunchDesktopEntry(_)
        | Action::OpenUrl { .. }
        | Action::RunScript { .. }
        | Action::System(_) => Effect::Desktop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petlauncher_core::{Mode, Row, RowKind, SystemCommand, TypedPath};

    fn directory_results(query: &str) -> Results {
        Results {
            query: query.to_owned(),
            mode: Mode::Path,
            rows: vec![
                Row::new(
                    RowKind::Directory,
                    "sub",
                    Action::SetQuery("~/d/sub/".into()),
                ),
                Row::new(
                    RowKind::File,
                    "a.txt",
                    Action::OpenFile("/h/d/a.txt".into()),
                ),
            ],
            selected: None,
            typed_path: Some(TypedPath::Directory("/h/d".into())),
        }
    }

    fn model_with(results: Results) -> LauncherModel {
        let mut model = LauncherModel::default();
        model.set_text(&results.query.clone());
        model.results_arrived(results);
        model
    }

    #[test]
    fn shows_the_results_of_the_text_now() {
        let mut model = LauncherModel::default();
        model.set_text("~/d/");

        let arrival = model.results_arrived(directory_results("~/d/"));

        assert_eq!(arrival, Arrival::Show);
        assert_eq!(model.results().rows.len(), 2);
    }

    #[test]
    fn drops_the_results_of_an_older_text() {
        let mut model = LauncherModel::default();
        model.set_text("~/d");
        model.set_text("~/d/");

        let arrival = model.results_arrived(directory_results("~/d"));

        assert_eq!(arrival, Arrival::Old);
        assert!(model.results().rows.is_empty());
    }

    #[test]
    fn a_typed_directory_starts_with_no_selected_row_and_down_selects_the_first() {
        let mut model = model_with(directory_results("~/d/"));
        let at_start = model.selected();

        model.down();

        assert_eq!(at_start, None);
        assert_eq!(model.selected(), Some(0));
    }

    #[test]
    fn new_results_take_the_selection_of_the_results() {
        let mut model = model_with(directory_results("~/d/"));
        model.down();
        model.down();

        model.results_arrived(directory_results("~/d/"));

        assert_eq!(model.selected(), None);
    }

    #[test]
    fn a_key_on_current_results_acts_now() {
        let mut model = model_with(directory_results("~/d/"));

        assert_eq!(model.press(Key::Enter), Press::Now);
    }

    #[test]
    fn a_key_before_the_results_of_the_text_waits_for_them() {
        let mut model = model_with(directory_results("~/d"));
        model.set_text("~/d/");

        let press = model.press(Key::CtrlEnter);
        let arrival = model.results_arrived(directory_results("~/d/"));

        assert_eq!(press, Press::Wait);
        assert_eq!(arrival, Arrival::ShowAndActivate(Key::CtrlEnter));
    }

    #[test]
    fn a_waiting_key_is_dropped_when_the_text_changes() {
        let mut model = LauncherModel::default();
        model.set_text("~/d/");
        model.press(Key::Enter);
        model.set_text("~/d/s");

        let arrival = model.results_arrived(directory_results("~/d/s"));

        assert_eq!(arrival, Arrival::Show);
    }

    #[test]
    fn clear_removes_the_text_and_the_rows() {
        let mut model = model_with(directory_results("~/d/"));

        model.clear();

        assert_eq!(model.text(), "");
        assert!(model.results().rows.is_empty());
        assert_eq!(model.press(Key::Enter), Press::Now);
    }

    #[test]
    fn restore_keeps_the_rows_of_the_kept_query() {
        let mut model = model_with(directory_results("~/d/"));

        model.restore("~/d/");

        assert_eq!(model.text(), "~/d/");
        assert_eq!(model.results().rows.len(), 2);
    }

    #[test]
    fn restore_removes_the_rows_of_a_different_text() {
        let mut model = model_with(directory_results("~/d/"));

        model.restore("lock");

        assert_eq!(model.text(), "lock");
        assert!(model.results().rows.is_empty());
        assert_eq!(model.selected(), None);
    }

    #[test]
    fn tab_completes_with_the_new_query_of_the_selected_row() {
        let mut model = model_with(directory_results("~/d/"));
        let with_no_selection = model.completion().map(str::to_owned);
        model.down();

        assert_eq!(with_no_selection, None);
        assert_eq!(model.completion(), Some("~/d/sub/"));
    }

    #[test]
    fn tab_does_nothing_on_a_row_that_does_not_change_the_query() {
        let mut model = model_with(directory_results("~/d/"));
        model.down();
        model.down();

        assert_eq!(model.completion(), None);
    }

    #[test]
    fn tab_does_nothing_on_the_rows_of_an_older_text() {
        let mut model = model_with(directory_results("~/d/"));
        model.down();
        model.set_text("~/d/x");

        assert_eq!(model.completion(), None);
    }

    #[test]
    fn select_follows_the_pointer() {
        let mut model = model_with(directory_results("~/d/"));

        model.select(1);
        model.up();

        assert_eq!(model.selected(), Some(0));
    }

    #[test]
    fn the_window_executes_query_clipboard_and_dictionary_actions() {
        assert_eq!(effect(&Action::Nothing), Effect::Nothing);
        assert_eq!(
            effect(&Action::SetQuery("~/a/".into())),
            Effect::SetQuery("~/a/")
        );
        assert_eq!(effect(&Action::Copy("125.6".into())), Effect::Copy("125.6"));
        assert_eq!(
            effect(&Action::OpenDictionary("launcher".into())),
            Effect::Dictionary("launcher")
        );
    }

    #[test]
    fn the_desktop_executes_the_other_actions() {
        let actions = [
            Action::OpenFile("/a".into()),
            Action::OpenFolder("/a".into()),
            Action::ShowInFolder("/a".into()),
            Action::LaunchDesktopEntry("/a.desktop".into()),
            Action::OpenUrl {
                url: "https://example.com".into(),
                browser: None,
            },
            Action::RunScript {
                script: "true".into(),
                argument: String::new(),
            },
            Action::System(SystemCommand::Lock),
        ];

        for action in &actions {
            assert_eq!(effect(action), Effect::Desktop, "{action:?}");
        }
    }
}
