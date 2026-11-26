//! The selected row of the result list, with the R1 state "no row selected".

/// The selected row. A result list that starts with no selected row (R1: the
/// contents of a typed directory) can go back to that state: Up on the first
/// row removes the selection, so that Enter acts on the typed path again. A
/// list that starts with a selected row always has one. The selection does
/// not wrap at the ends of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    len: usize,
    selected: Option<usize>,
    allow_none: bool,
}

impl Selection {
    /// `selected` is the selection at the start, from `Results::selected`.
    pub fn new(len: usize, selected: Option<usize>) -> Self {
        let selected = selected.filter(|&index| index < len);
        Self {
            len,
            selected,
            allow_none: selected.is_none(),
        }
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn down(&mut self) {
        self.selected = match self.selected {
            _ if self.len == 0 => None,
            None => Some(0),
            Some(index) => Some((index + 1).min(self.len - 1)),
        };
    }

    pub fn up(&mut self) {
        self.selected = match self.selected {
            Some(0) if self.allow_none => None,
            Some(index) => Some(index.saturating_sub(1)),
            None => None,
        };
    }

    /// Selects a row that the pointer activates. An index out of the list
    /// changes nothing.
    pub fn select(&mut self, index: usize) {
        if index < self.len {
            self.selected = Some(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_directory_starts_with_no_selected_row() {
        let selection = Selection::new(3, None);

        assert_eq!(selection.selected(), None);
    }

    #[test]
    fn down_with_no_selected_row_selects_the_first_row() {
        let mut selection = Selection::new(3, None);

        selection.down();

        assert_eq!(selection.selected(), Some(0));
    }

    #[test]
    fn up_on_the_first_row_goes_back_to_no_selected_row() {
        let mut selection = Selection::new(3, None);
        selection.down();

        selection.up();

        assert_eq!(selection.selected(), None);
    }

    #[test]
    fn up_with_no_selected_row_keeps_no_selected_row() {
        let mut selection = Selection::new(3, None);

        selection.up();

        assert_eq!(selection.selected(), None);
    }

    #[test]
    fn a_list_that_starts_with_a_selected_row_keeps_one_at_the_top() {
        let mut selection = Selection::new(3, Some(0));

        selection.up();

        assert_eq!(selection.selected(), Some(0));
    }

    #[test]
    fn down_stops_at_the_last_row() {
        let mut selection = Selection::new(2, Some(0));

        selection.down();
        selection.down();

        assert_eq!(selection.selected(), Some(1));
    }

    #[test]
    fn up_and_down_move_one_row() {
        let mut selection = Selection::new(5, Some(2));

        selection.down();
        let after_down = selection.selected();
        selection.up();
        selection.up();

        assert_eq!(after_down, Some(3));
        assert_eq!(selection.selected(), Some(1));
    }

    #[test]
    fn an_empty_list_has_no_selected_row() {
        let mut selection = Selection::new(0, Some(0));

        selection.down();

        assert_eq!(selection.selected(), None);
    }

    #[test]
    fn select_accepts_only_a_row_of_the_list() {
        let mut selection = Selection::new(2, None);

        selection.select(5);
        let out_of_list = selection.selected();
        selection.select(1);

        assert_eq!(out_of_list, None);
        assert_eq!(selection.selected(), Some(1));
    }
}
