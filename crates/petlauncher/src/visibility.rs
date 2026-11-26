//! When the launcher window shows and hides, and why it hides (R3, R9).

use petlauncher_core::CloseReason;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Phase {
    #[default]
    Hidden,
    /// `present()` was called, and the window did not have the focus yet.
    Opening,
    /// The window had the focus after the show.
    Open,
}

/// The show state of the launcher window.
///
/// "Close petlauncher when losing focus" (R3) hides the window only after
/// the window had the focus once since the show. A window that shows is not
/// focused until GNOME Shell activates it, so an unfocused state at that
/// time is not a focus loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Visibility {
    phase: Phase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    Show,
    Hide(CloseReason),
}

impl Visibility {
    pub fn is_visible(&self) -> bool {
        self.phase != Phase::Hidden
    }

    /// The program shows the window. Gives false when the window shows
    /// already: then the query and the rows must not change.
    pub fn show(&mut self) -> bool {
        if self.is_visible() {
            return false;
        }
        self.phase = Phase::Opening;
        true
    }

    /// The window got the keyboard focus. Gives true for the first focus
    /// after a show.
    pub fn activated(&mut self) -> bool {
        let first = self.phase == Phase::Opening;
        if first {
            self.phase = Phase::Open;
        }
        first
    }

    /// The window lost the keyboard focus. Gives the reason to hide the
    /// window, or `None` when it stays.
    pub fn deactivated(&self, close_on_focus_loss: bool) -> Option<CloseReason> {
        (self.phase == Phase::Open && close_on_focus_loss).then_some(CloseReason::FocusLoss)
    }

    /// The program hides the window. Gives false when the window is hidden
    /// already.
    pub fn hide(&mut self) -> bool {
        let was_visible = self.is_visible();
        self.phase = Phase::Hidden;
        was_visible
    }

    /// `petlauncher toggle`, which the global key runs (R4).
    pub fn toggle(&self) -> Toggle {
        if self.is_visible() {
            Toggle::Hide(CloseReason::GlobalKey)
        } else {
            Toggle::Show
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_hidden() {
        assert!(!Visibility::default().is_visible());
    }

    #[test]
    fn an_inactive_window_during_the_show_stays() {
        let mut visibility = Visibility::default();
        visibility.show();

        let reason = visibility.deactivated(true);

        assert_eq!(reason, None);
        assert!(visibility.is_visible());
    }

    #[test]
    fn a_focus_loss_after_the_focus_hides_the_window() {
        let mut visibility = Visibility::default();
        visibility.show();
        visibility.activated();

        let reason = visibility.deactivated(true);

        assert_eq!(reason, Some(CloseReason::FocusLoss));
    }

    #[test]
    fn a_focus_loss_keeps_the_window_when_the_setting_is_off() {
        let mut visibility = Visibility::default();
        visibility.show();
        visibility.activated();

        let reason = visibility.deactivated(false);

        assert_eq!(reason, None);
    }

    #[test]
    fn a_focus_loss_of_a_hidden_window_does_nothing() {
        let mut visibility = Visibility::default();
        visibility.show();
        visibility.activated();
        visibility.hide();

        let reason = visibility.deactivated(true);

        assert_eq!(reason, None);
    }

    #[test]
    fn each_show_waits_again_for_the_focus() {
        let mut visibility = Visibility::default();
        visibility.show();
        visibility.activated();
        visibility.hide();
        visibility.show();

        let reason = visibility.deactivated(true);

        assert_eq!(reason, None);
    }

    #[test]
    fn only_the_first_focus_after_a_show_is_first() {
        let mut visibility = Visibility::default();
        visibility.show();

        let first = visibility.activated();
        let second = visibility.activated();

        assert!(first);
        assert!(!second);
    }

    #[test]
    fn a_second_show_changes_nothing() {
        let mut visibility = Visibility::default();
        visibility.show();
        visibility.activated();

        let shown_again = visibility.show();

        assert!(!shown_again);
        assert_eq!(visibility.deactivated(true), Some(CloseReason::FocusLoss));
    }

    #[test]
    fn a_focus_before_the_show_does_nothing() {
        let mut visibility = Visibility::default();
        visibility.activated();
        visibility.show();

        assert_eq!(visibility.deactivated(true), None);
    }

    #[test]
    fn hide_tells_if_the_window_was_visible() {
        let mut visibility = Visibility::default();
        visibility.show();

        assert!(visibility.hide());
        assert!(!visibility.hide());
    }

    #[test]
    fn toggle_shows_a_hidden_window_and_hides_a_visible_window() {
        let mut visibility = Visibility::default();
        let when_hidden = visibility.toggle();
        visibility.show();

        let when_opening = visibility.toggle();
        visibility.activated();
        let when_open = visibility.toggle();

        assert_eq!(when_hidden, Toggle::Show);
        assert_eq!(when_opening, Toggle::Hide(CloseReason::GlobalKey));
        assert_eq!(when_open, Toggle::Hide(CloseReason::GlobalKey));
    }
}
