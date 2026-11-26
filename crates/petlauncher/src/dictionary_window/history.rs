//! The words that the dictionary window showed, for the back and forward
//! buttons (R19 step 5).

/// One shown word and the scroll position of its page when the window left
/// it.
#[derive(Debug, Clone, PartialEq)]
struct Visit {
    word: String,
    scroll: f64,
}

/// The history of a browser: a visit after a back removes the words after
/// the current word.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct History {
    visits: Vec<Visit>,
    current: usize,
}

impl History {
    /// The word that the window shows, or `None` before the first visit.
    pub fn current(&self) -> Option<&str> {
        self.visits
            .get(self.current)
            .map(|visit| visit.word.as_str())
    }

    /// The scroll position to restore when the window shows the current word
    /// again after back or forward. 0 for a new visit.
    pub fn scroll(&self) -> f64 {
        self.visits
            .get(self.current)
            .map_or(0.0, |visit| visit.scroll)
    }

    /// Keeps the scroll position of the current word before the window
    /// leaves it.
    pub fn set_scroll(&mut self, scroll: f64) {
        if let Some(visit) = self.visits.get_mut(self.current) {
            visit.scroll = scroll;
        }
    }

    /// Makes the word the current word. A visit to the current word adds
    /// nothing. Gives true when the history changed.
    pub fn visit(&mut self, word: &str) -> bool {
        if self.current() == Some(word) {
            return false;
        }
        if !self.visits.is_empty() {
            self.visits.truncate(self.current + 1);
        }
        self.visits.push(Visit {
            word: word.to_owned(),
            scroll: 0.0,
        });
        self.current = self.visits.len() - 1;
        true
    }

    pub fn can_go_back(&self) -> bool {
        self.current > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.current + 1 < self.visits.len()
    }

    /// Goes to the word before the current word. Gives false at the first
    /// word.
    pub fn back(&mut self) -> bool {
        let can = self.can_go_back();
        if can {
            self.current -= 1;
        }
        can
    }

    /// Goes to the word after the current word. Gives false at the last
    /// word.
    pub fn forward(&mut self) -> bool {
        let can = self.can_go_forward();
        if can {
            self.current += 1;
        }
        can
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(words: &[&str]) -> History {
        let mut history = History::default();
        for word in words {
            history.visit(word);
        }
        history
    }

    #[test]
    fn an_empty_history_has_no_word_and_no_way_back() {
        let mut history = History::default();

        assert_eq!(history.current(), None);
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
        assert!(!history.back());
        assert!(!history.forward());
        assert_eq!(history.scroll(), 0.0);
    }

    #[test]
    fn a_visit_makes_the_word_current() {
        let history = history(&["launcher", "rocket launcher"]);

        assert_eq!(history.current(), Some("rocket launcher"));
        assert!(history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn back_shows_the_earlier_word_and_forward_the_later_word() {
        let mut history = history(&["launcher", "rocket launcher"]);

        let went_back = history.back();
        let after_back = history.current().map(str::to_owned);
        let went_forward = history.forward();

        assert!(went_back);
        assert_eq!(after_back.as_deref(), Some("launcher"));
        assert!(went_forward);
        assert_eq!(history.current(), Some("rocket launcher"));
    }

    #[test]
    fn back_stops_at_the_first_word() {
        let mut history = history(&["launcher"]);

        assert!(!history.back());
        assert_eq!(history.current(), Some("launcher"));
    }

    #[test]
    fn a_visit_after_back_removes_the_forward_part() {
        let mut history = history(&["launcher", "rocket launcher", "bazooka"]);
        history.back();
        history.back();

        history.visit("catapult");

        assert_eq!(history.current(), Some("catapult"));
        assert!(!history.can_go_forward());
        history.back();
        assert_eq!(history.current(), Some("launcher"));
        assert!(!history.can_go_back());
    }

    #[test]
    fn a_visit_to_the_current_word_adds_nothing() {
        let mut history = history(&["launcher", "rocket launcher"]);
        history.back();

        let changed = history.visit("launcher");

        assert!(!changed);
        assert!(history.can_go_forward());
        assert!(!history.can_go_back());
    }

    #[test]
    fn a_visit_to_an_earlier_word_that_is_not_current_is_a_new_visit() {
        let mut history = history(&["launcher", "rocket launcher"]);

        let changed = history.visit("launcher");

        assert!(changed);
        history.back();
        assert_eq!(history.current(), Some("rocket launcher"));
    }

    #[test]
    fn keeps_the_scroll_position_of_each_word() {
        let mut history = history(&["run"]);
        history.set_scroll(840.0);
        history.visit("operate");
        let new_visit = history.scroll();

        history.back();

        assert_eq!(new_visit, 0.0);
        assert_eq!(history.scroll(), 840.0);
    }
}
