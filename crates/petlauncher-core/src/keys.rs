//! Enter, Ctrl+Enter and Shift+Enter on the selected row (R1, R2, R13).

use std::path::Path;

use crate::contract::{Action, Key, Outcome, Results, Row, RowKind, TypedPath};

/// Gives what the program does when the owner presses `key`.
///
/// `selected` is the row that the user interface has selected now, which
/// can differ from `results.selected` after the arrow keys. With no
/// selected row, the keys act on the typed path of the path mode. With no
/// typed path either, the keys do nothing.
pub fn activate(results: &Results, selected: Option<usize>, key: Key) -> Outcome {
    if let Some(row) = selected.and_then(|index| results.rows.get(index)) {
        return activate_row(row, key);
    }
    match &results.typed_path {
        Some(typed_path) => activate_typed_path(typed_path, key),
        None => outcome(Action::Nothing, false),
    }
}

fn activate_row(row: &Row, key: Key) -> Outcome {
    let action = match (key, &row.local_file) {
        (Key::Enter, _) => row.enter.clone(),
        (_, None) => Action::Nothing,
        (Key::CtrlEnter, Some(path)) if row.kind == RowKind::Directory => {
            Action::OpenFolder(path.clone())
        }
        (Key::CtrlEnter, Some(path)) => Action::ShowInFolder(path.clone()),
        (Key::ShiftEnter, Some(path)) => copy_path(path),
    };
    outcome(
        action,
        key == Key::Enter && row.kind == RowKind::Calculation,
    )
}

fn activate_typed_path(typed_path: &TypedPath, key: Key) -> Outcome {
    let action = match (typed_path, key) {
        (TypedPath::Directory(path), Key::Enter | Key::CtrlEnter) => {
            Action::OpenFolder(path.clone())
        }
        (TypedPath::File(path), Key::Enter) => Action::OpenFile(path.clone()),
        (TypedPath::File(path), Key::CtrlEnter) => Action::ShowInFolder(path.clone()),
        (_, Key::ShiftEnter) => copy_path(typed_path.path()),
    };
    outcome(action, false)
}

fn copy_path(path: &Path) -> Action {
    Action::Copy(path.to_string_lossy().into_owned())
}

fn outcome(action: Action, keep_query: bool) -> Outcome {
    let close = action.closes_window();
    Outcome {
        action,
        close,
        keep_query: close && keep_query,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{Action, Mode, Row, RowKind, TypedPath};
    use std::path::PathBuf;

    fn results(rows: Vec<Row>, typed_path: Option<TypedPath>) -> Results {
        Results {
            query: "query".to_owned(),
            mode: Mode::General,
            selected: if rows.is_empty() { None } else { Some(0) },
            rows,
            typed_path,
        }
    }

    fn file_row(path: &str) -> Row {
        Row::new(RowKind::File, "file", Action::OpenFile(path.into())).with_local_file(path)
    }

    fn directory_row(path: &str) -> Row {
        Row::new(
            RowKind::Directory,
            "dir",
            Action::SetQuery(format!("{path}/")),
        )
        .with_local_file(path)
    }

    fn calculation_row() -> Row {
        Row::new(RowKind::Calculation, "125.6", Action::Copy("125.6".into()))
    }

    #[test]
    fn enter_on_a_row_gives_its_primary_action_and_closes() {
        let results = results(vec![file_row("/a/b.png")], None);

        let outcome = activate(&results, Some(0), Key::Enter);

        assert_eq!(outcome.action, Action::OpenFile("/a/b.png".into()));
        assert!(outcome.close);
        assert!(!outcome.keep_query);
    }

    #[test]
    fn enter_on_a_calculation_keeps_the_query() {
        let results = results(vec![calculation_row()], None);

        let outcome = activate(&results, Some(0), Key::Enter);

        assert_eq!(outcome.action, Action::Copy("125.6".into()));
        assert!(outcome.close);
        assert!(outcome.keep_query);
    }

    #[test]
    fn enter_that_sets_the_query_keeps_the_window_open() {
        let results = results(vec![directory_row("/a/dir")], None);

        let outcome = activate(&results, Some(0), Key::Enter);

        assert_eq!(outcome.action, Action::SetQuery("/a/dir/".into()));
        assert!(!outcome.close);
    }

    #[test]
    fn ctrl_enter_on_a_file_row_shows_the_file_in_its_folder() {
        let results = results(vec![file_row("/a/b.png")], None);

        let outcome = activate(&results, Some(0), Key::CtrlEnter);

        assert_eq!(outcome.action, Action::ShowInFolder("/a/b.png".into()));
        assert!(outcome.close);
        assert!(!outcome.keep_query);
    }

    #[test]
    fn ctrl_enter_on_a_directory_row_opens_the_directory() {
        let results = results(vec![directory_row("/a/dir")], None);

        let outcome = activate(&results, Some(0), Key::CtrlEnter);

        assert_eq!(outcome.action, Action::OpenFolder("/a/dir".into()));
        assert!(outcome.close);
    }

    #[test]
    fn ctrl_enter_and_shift_enter_on_a_row_with_no_local_file_do_nothing() {
        let results = results(vec![calculation_row()], None);

        for key in [Key::CtrlEnter, Key::ShiftEnter] {
            let outcome = activate(&results, Some(0), key);

            assert_eq!(outcome.action, Action::Nothing);
            assert!(!outcome.close);
            assert!(!outcome.keep_query);
        }
    }

    #[test]
    fn shift_enter_copies_the_path_of_the_local_file() {
        let results = results(vec![directory_row("/a/dir")], None);

        let outcome = activate(&results, Some(0), Key::ShiftEnter);

        assert_eq!(outcome.action, Action::Copy("/a/dir".into()));
        assert!(outcome.close);
    }

    #[test]
    fn keys_on_a_typed_directory_with_no_selected_row_act_on_the_directory() {
        let dir = PathBuf::from("/a/dir");
        let mut results = results(
            vec![file_row("/a/dir/b.png")],
            Some(TypedPath::Directory(dir.clone())),
        );
        results.selected = None;

        let enter = activate(&results, None, Key::Enter);
        let ctrl = activate(&results, None, Key::CtrlEnter);
        let shift = activate(&results, None, Key::ShiftEnter);

        assert_eq!(enter.action, Action::OpenFolder(dir.clone()));
        assert_eq!(ctrl.action, Action::OpenFolder(dir));
        assert_eq!(shift.action, Action::Copy("/a/dir".into()));
        assert!(enter.close && ctrl.close && shift.close);
    }

    #[test]
    fn keys_on_a_typed_file_with_no_selected_row_act_on_the_file() {
        let file = PathBuf::from("/a/b.png");
        let results = results(Vec::new(), Some(TypedPath::File(file.clone())));

        assert_eq!(
            activate(&results, None, Key::Enter).action,
            Action::OpenFile(file.clone())
        );
        assert_eq!(
            activate(&results, None, Key::CtrlEnter).action,
            Action::ShowInFolder(file)
        );
        assert_eq!(
            activate(&results, None, Key::ShiftEnter).action,
            Action::Copy("/a/b.png".into())
        );
    }

    #[test]
    fn keys_with_no_selected_row_and_no_typed_path_do_nothing() {
        let results = results(vec![calculation_row()], None);

        for key in [Key::Enter, Key::CtrlEnter, Key::ShiftEnter] {
            let outcome = activate(&results, None, key);

            assert_eq!(outcome.action, Action::Nothing);
            assert!(!outcome.close);
        }
    }

    #[test]
    fn a_selected_index_outside_the_rows_counts_as_no_selected_row() {
        let results = results(vec![calculation_row()], None);

        let outcome = activate(&results, Some(5), Key::Enter);

        assert_eq!(outcome.action, Action::Nothing);
    }
}
