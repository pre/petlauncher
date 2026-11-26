//! The keys of the launcher window and what they do.

use gtk::gdk;
use petlauncher_core::Key;

/// What a key press does in the launcher window. The keys that give no
/// intent go to the query line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// Enter, Ctrl+Enter or Shift+Enter: `activate` of the core.
    Activate(Key),
    Up,
    Down,
    /// Escape: hide the window.
    Hide,
    /// Tab: complete the query with the selected row.
    Complete,
    /// Ctrl+comma: open the settings window, as in many GNOME applications.
    Settings,
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

pub fn intent(keyval: gdk::Key, state: gdk::ModifierType) -> Option<Intent> {
    let modifiers = modifiers(state);
    let none = modifiers.is_empty();
    match keyval {
        gdk::Key::Return | gdk::Key::KP_Enter | gdk::Key::ISO_Enter => {
            if none {
                Some(Intent::Activate(Key::Enter))
            } else if modifiers == gdk::ModifierType::CONTROL_MASK {
                Some(Intent::Activate(Key::CtrlEnter))
            } else if modifiers == gdk::ModifierType::SHIFT_MASK {
                Some(Intent::Activate(Key::ShiftEnter))
            } else {
                None
            }
        }
        gdk::Key::Up | gdk::Key::KP_Up if none => Some(Intent::Up),
        gdk::Key::Down | gdk::Key::KP_Down if none => Some(Intent::Down),
        gdk::Key::Escape if none => Some(Intent::Hide),
        gdk::Key::Tab if none => Some(Intent::Complete),
        gdk::Key::comma if modifiers == gdk::ModifierType::CONTROL_MASK => Some(Intent::Settings),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gdk::ModifierType as M;

    #[test]
    fn enter_with_no_modifier_is_enter() {
        for keyval in [gdk::Key::Return, gdk::Key::KP_Enter, gdk::Key::ISO_Enter] {
            assert_eq!(
                intent(keyval, M::empty()),
                Some(Intent::Activate(Key::Enter))
            );
        }
    }

    #[test]
    fn ctrl_comma_opens_the_settings_and_a_comma_is_text() {
        assert_eq!(
            intent(gdk::Key::comma, M::CONTROL_MASK),
            Some(Intent::Settings)
        );
        assert_eq!(intent(gdk::Key::comma, M::empty()), None);
        assert_eq!(
            intent(gdk::Key::comma, M::CONTROL_MASK | M::SHIFT_MASK),
            None
        );
    }

    #[test]
    fn ctrl_and_shift_change_enter() {
        assert_eq!(
            intent(gdk::Key::Return, M::CONTROL_MASK),
            Some(Intent::Activate(Key::CtrlEnter))
        );
        assert_eq!(
            intent(gdk::Key::KP_Enter, M::SHIFT_MASK),
            Some(Intent::Activate(Key::ShiftEnter))
        );
    }

    #[test]
    fn enter_with_other_modifiers_does_nothing() {
        for state in [M::ALT_MASK, M::SUPER_MASK, M::CONTROL_MASK | M::SHIFT_MASK] {
            assert_eq!(intent(gdk::Key::Return, state), None, "{state:?}");
        }
    }

    #[test]
    fn caps_lock_and_num_lock_do_not_change_a_key() {
        let locks = M::LOCK_MASK | M::from_bits_retain(0x10);

        assert_eq!(
            intent(gdk::Key::Return, locks | M::CONTROL_MASK),
            Some(Intent::Activate(Key::CtrlEnter))
        );
        assert_eq!(intent(gdk::Key::Down, locks), Some(Intent::Down));
    }

    #[test]
    fn arrows_move_the_selection() {
        assert_eq!(intent(gdk::Key::Up, M::empty()), Some(Intent::Up));
        assert_eq!(intent(gdk::Key::KP_Up, M::empty()), Some(Intent::Up));
        assert_eq!(intent(gdk::Key::Down, M::empty()), Some(Intent::Down));
        assert_eq!(intent(gdk::Key::KP_Down, M::empty()), Some(Intent::Down));
    }

    #[test]
    fn escape_hides_and_tab_completes() {
        assert_eq!(intent(gdk::Key::Escape, M::empty()), Some(Intent::Hide));
        assert_eq!(intent(gdk::Key::Tab, M::empty()), Some(Intent::Complete));
    }

    #[test]
    fn other_keys_go_to_the_query_line() {
        assert_eq!(intent(gdk::Key::a, M::empty()), None);
        assert_eq!(intent(gdk::Key::Left, M::empty()), None);
        assert_eq!(intent(gdk::Key::Up, M::SHIFT_MASK), None);
        assert_eq!(intent(gdk::Key::Tab, M::CONTROL_MASK), None);
    }
}
