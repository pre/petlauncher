//! The keys of the dictionary window and what they do.

use gtk::gdk;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Ctrl+F or Ctrl+L: put the keyboard focus in the search field.
    FocusSearch,
    /// Alt+Left: the earlier word.
    Back,
    /// Alt+Right: the later word.
    Forward,
    /// Escape: see `escape`.
    Escape,
    /// Up and Down: move in the suggestions, else scroll the page.
    Up,
    Down,
    /// Page Up and Page Down: scroll one page.
    PageUp,
    PageDown,
}

/// What Escape does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escape {
    /// The search field has text: Escape removes it, and the window shows
    /// the word again.
    ClearField,
    /// The search field is empty: Escape closes the window, as in the
    /// launcher.
    Close,
}

pub fn escape(field_text: &str) -> Escape {
    if field_text.is_empty() {
        Escape::Close
    } else {
        Escape::ClearField
    }
}

/// The modifiers that change the meaning of a key. Caps Lock and Num Lock do
/// not.
fn modifiers(state: gdk::ModifierType) -> gdk::ModifierType {
    state
        & (gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::SHIFT_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK
            | gdk::ModifierType::HYPER_MASK
            | gdk::ModifierType::META_MASK)
}

/// The intent of a key. `None`: the key goes to the widget with the focus,
/// for example Ctrl+C to a label with selected text.
pub fn intent(keyval: gdk::Key, state: gdk::ModifierType) -> Option<Intent> {
    let modifiers = modifiers(state);
    let none = modifiers.is_empty();
    let control = modifiers == gdk::ModifierType::CONTROL_MASK;
    let alt = modifiers == gdk::ModifierType::ALT_MASK;
    match keyval.to_lower() {
        gdk::Key::f | gdk::Key::l if control => Some(Intent::FocusSearch),
        gdk::Key::Left | gdk::Key::KP_Left if alt => Some(Intent::Back),
        gdk::Key::Right | gdk::Key::KP_Right if alt => Some(Intent::Forward),
        gdk::Key::Escape if none => Some(Intent::Escape),
        gdk::Key::Up | gdk::Key::KP_Up if none => Some(Intent::Up),
        gdk::Key::Down | gdk::Key::KP_Down if none => Some(Intent::Down),
        gdk::Key::Page_Up | gdk::Key::KP_Page_Up if none => Some(Intent::PageUp),
        gdk::Key::Page_Down | gdk::Key::KP_Page_Down if none => Some(Intent::PageDown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gdk::ModifierType as M;

    #[test]
    fn control_f_and_control_l_focus_the_search_field() {
        assert_eq!(
            intent(gdk::Key::f, M::CONTROL_MASK),
            Some(Intent::FocusSearch)
        );
        assert_eq!(
            intent(gdk::Key::l, M::CONTROL_MASK),
            Some(Intent::FocusSearch)
        );
        assert_eq!(
            intent(gdk::Key::F, M::CONTROL_MASK | M::LOCK_MASK),
            Some(Intent::FocusSearch)
        );
    }

    #[test]
    fn alt_left_and_alt_right_go_back_and_forward() {
        assert_eq!(intent(gdk::Key::Left, M::ALT_MASK), Some(Intent::Back));
        assert_eq!(intent(gdk::Key::Right, M::ALT_MASK), Some(Intent::Forward));
    }

    #[test]
    fn plain_arrows_move_and_scroll() {
        assert_eq!(intent(gdk::Key::Down, M::empty()), Some(Intent::Down));
        assert_eq!(intent(gdk::Key::Up, M::empty()), Some(Intent::Up));
        assert_eq!(
            intent(gdk::Key::Page_Down, M::empty()),
            Some(Intent::PageDown)
        );
        assert_eq!(intent(gdk::Key::Page_Up, M::empty()), Some(Intent::PageUp));
    }

    #[test]
    fn keys_of_the_text_go_to_the_focused_widget() {
        assert_eq!(intent(gdk::Key::Left, M::empty()), None);
        assert_eq!(intent(gdk::Key::c, M::CONTROL_MASK), None);
        assert_eq!(intent(gdk::Key::a, M::CONTROL_MASK), None);
        assert_eq!(intent(gdk::Key::Down, M::SHIFT_MASK), None);
        assert_eq!(intent(gdk::Key::f, M::empty()), None);
    }

    #[test]
    fn escape_is_an_intent_with_no_modifier_only() {
        assert_eq!(intent(gdk::Key::Escape, M::empty()), Some(Intent::Escape));
        assert_eq!(intent(gdk::Key::Escape, M::SHIFT_MASK), None);
    }

    #[test]
    fn escape_clears_a_field_with_text_and_closes_with_an_empty_field() {
        assert_eq!(escape("launc"), Escape::ClearField);
        assert_eq!(escape(" "), Escape::ClearField);
        assert_eq!(escape(""), Escape::Close);
    }
}
