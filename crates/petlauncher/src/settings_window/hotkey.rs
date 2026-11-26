//! R4: the state of the global key in the settings window, with no
//! widgets. The window shows the key that GNOME has, a candidate key that
//! the owner presses or types, the error of the text, and each other GNOME
//! key binding with the candidate key (ADR 0008).

use gtk::gdk;
use petlauncher_gnome::{validate_accelerator, Conflict, CustomShortcuts, TOGGLE_COMMAND};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HotkeyState {
    /// The key of the petlauncher shortcut in GNOME now. `None`: no key.
    pub current: Option<String>,
    /// The key that the owner wants.
    pub candidate: String,
    /// The error of the candidate text, for the owner.
    pub error: Option<String>,
    /// The other key bindings with the candidate key.
    pub conflicts: Vec<Conflict>,
}

impl HotkeyState {
    /// Reads the key from GNOME. The candidate is that key, or the setting
    /// `hotkey` when GNOME has no key.
    pub fn load(shortcuts: &CustomShortcuts, hotkey_setting: &str) -> Self {
        let current = shortcuts.global_key();
        let mut state = Self {
            candidate: current
                .clone()
                .unwrap_or_else(|| hotkey_setting.trim().to_owned()),
            current,
            ..Self::default()
        };
        state.check(shortcuts);
        state
    }

    /// A new candidate: pressed or typed.
    pub fn set_candidate(&mut self, shortcuts: &CustomShortcuts, text: &str) {
        self.candidate = text.trim().to_owned();
        self.check(shortcuts);
    }

    /// Checks the text and reads the conflicts again, for example after
    /// "Remove".
    pub fn check(&mut self, shortcuts: &CustomShortcuts) {
        self.conflicts.clear();
        self.error = None;
        if self.candidate.is_empty() {
            return;
        }
        if let Err(error) = validate_accelerator(&self.candidate) {
            self.error = Some(error.to_string());
            return;
        }
        match shortcuts.conflicts(&self.candidate) {
            Ok(conflicts) => self.conflicts = conflicts,
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// "Set" is active for a correct key that is not the key of GNOME now
    /// and that no other key binding has.
    pub fn can_set(&self) -> bool {
        !self.candidate.is_empty()
            && self.error.is_none()
            && self.conflicts.is_empty()
            && self.current.as_deref() != Some(self.candidate.as_str())
    }

    /// "Remove the key" is active when petlauncher has a key.
    pub fn can_clear(&self) -> bool {
        self.current.is_some()
    }

    /// The text below the key, for the owner. `label` gives the key as
    /// GNOME Settings shows it, for example "Super+Space".
    pub fn status(&self, label: impl Fn(&str) -> String) -> String {
        if let Some(error) = &self.error {
            return error.clone();
        }
        if self.candidate.is_empty() {
            return "Press a key or type it, for example <Super>space.".into();
        }
        let key = label(&self.candidate);
        if !self.conflicts.is_empty() {
            return format!(
                "{key} is the key of the key bindings below. Remove it from each of them, \
                 then select Set."
            );
        }
        match &self.current {
            Some(current) if *current == self.candidate => {
                format!("GNOME runs \"{TOGGLE_COMMAND}\" with {key}.")
            }
            _ => format!("Select Set to use {key} for petlauncher."),
        }
    }

    /// Removes the key of one conflict, after the "Remove" button.
    pub fn remove_conflict(
        &mut self,
        shortcuts: &CustomShortcuts,
        index: usize,
    ) -> Result<(), String> {
        let conflict = self
            .conflicts
            .get(index)
            .cloned()
            .ok_or_else(|| "This key binding is not in the list any more.".to_owned())?;
        shortcuts
            .remove_key(&conflict)
            .map_err(|error| error.to_string())?;
        self.check(shortcuts);
        Ok(())
    }

    /// Writes the candidate key, only when no conflict is left. Gives true
    /// when GNOME has the key now.
    pub fn set(&mut self, shortcuts: &CustomShortcuts) -> Result<bool, String> {
        if !self.can_set() {
            return Ok(false);
        }
        let conflicts = shortcuts
            .set_global_key_if_free(&self.candidate, TOGGLE_COMMAND)
            .map_err(|error| error.to_string())?;
        if !conflicts.is_empty() {
            self.conflicts = conflicts;
            return Ok(false);
        }
        self.current = Some(self.candidate.clone());
        Ok(true)
    }

    /// Removes the key of petlauncher. The shortcut stays with no key.
    pub fn clear(&mut self, shortcuts: &CustomShortcuts) -> Result<(), String> {
        shortcuts
            .set_global_key("", TOGGLE_COMMAND)
            .map_err(|error| error.to_string())?;
        self.current = None;
        self.candidate.clear();
        self.check(shortcuts);
        Ok(())
    }
}

/// What a key press does while the window waits for a new key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capture {
    /// A modifier key alone: wait for the next key.
    Wait,
    /// Escape with no modifier: stop waiting.
    Cancel,
    /// The key as an accelerator text, for example `<Super>space`.
    Key(String),
}

/// The accelerator of a key press, in the order Control, Alt, Shift,
/// Super. A letter is written in small letters, as GNOME does.
pub fn capture(keyval: gdk::Key, state: gdk::ModifierType) -> Capture {
    use gdk::ModifierType as M;
    if is_modifier(keyval) {
        return Capture::Wait;
    }
    let modifiers = state & (M::CONTROL_MASK | M::ALT_MASK | M::SHIFT_MASK | M::SUPER_MASK);
    if keyval == gdk::Key::Escape && modifiers.is_empty() {
        return Capture::Cancel;
    }
    let mut text = String::new();
    for (mask, name) in [
        (M::CONTROL_MASK, "<Control>"),
        (M::ALT_MASK, "<Alt>"),
        (M::SHIFT_MASK, "<Shift>"),
        (M::SUPER_MASK, "<Super>"),
    ] {
        if modifiers.contains(mask) {
            text.push_str(name);
        }
    }
    let name = keyval
        .to_lower()
        .name()
        .map(|name| name.to_string())
        .unwrap_or_default();
    text.push_str(&name);
    Capture::Key(text)
}

fn is_modifier(keyval: gdk::Key) -> bool {
    matches!(
        keyval,
        gdk::Key::Shift_L
            | gdk::Key::Shift_R
            | gdk::Key::Control_L
            | gdk::Key::Control_R
            | gdk::Key::Alt_L
            | gdk::Key::Alt_R
            | gdk::Key::Super_L
            | gdk::Key::Super_R
            | gdk::Key::Meta_L
            | gdk::Key::Meta_R
            | gdk::Key::Hyper_L
            | gdk::Key::Hyper_R
            | gdk::Key::ISO_Level3_Shift
            | gdk::Key::Caps_Lock
            | gdk::Key::Num_Lock
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{SettingsBackendFixture, NOTES};
    use gdk::ModifierType as M;
    use petlauncher_gnome::BindingKind;

    #[test]
    fn loads_the_key_of_gnome_and_its_conflicts() {
        let fixture = SettingsBackendFixture::with_conflicts();

        let state = HotkeyState::load(&fixture.shortcuts(), "<Super>space");

        assert_eq!(state.current, None);
        assert_eq!(state.candidate, "<Super>space");
        assert_eq!(state.conflicts.len(), 2);
        assert!(!state.can_set());
        assert!(!state.can_clear());
        assert!(
            state
                .status(str::to_owned)
                .contains("Remove it from each of them"),
            "{}",
            state.status(str::to_owned)
        );
    }

    #[test]
    fn remove_takes_the_key_from_one_binding_and_set_becomes_active_with_no_conflict() {
        let fixture = SettingsBackendFixture::with_conflicts();
        let shortcuts = fixture.shortcuts();
        let mut state = HotkeyState::load(&shortcuts, "<Super>space");

        state.remove_conflict(&shortcuts, 0).unwrap();
        let after_one = state.conflicts.len();
        state.remove_conflict(&shortcuts, 0).unwrap();

        assert_eq!(after_one, 1);
        assert!(state.conflicts.is_empty());
        assert!(state.can_set());
        assert_eq!(fixture.binding(NOTES), "");
        assert_eq!(fixture.input_source_keys(), ["XF86Keyboard"]);
    }

    #[test]
    fn set_writes_the_key_only_with_no_conflict() {
        let fixture = SettingsBackendFixture::with_conflicts();
        let shortcuts = fixture.shortcuts();
        let mut state = HotkeyState::load(&shortcuts, "<Super>space");

        let refused = state.set(&shortcuts).unwrap();
        state.set_candidate(&shortcuts, "<Control><Super>k");
        let written = state.set(&shortcuts).unwrap();

        assert!(!refused);
        assert!(written);
        assert_eq!(shortcuts.global_key().as_deref(), Some("<Control><Super>k"));
        assert_eq!(state.current.as_deref(), Some("<Control><Super>k"));
        assert!(!state.can_set());
        assert!(
            state.status(str::to_owned).contains("GNOME runs"),
            "{}",
            state.status(str::to_owned)
        );
    }

    #[test]
    fn a_wrong_text_shows_its_error_and_set_stays_off() {
        let fixture = SettingsBackendFixture::with_conflicts();
        let shortcuts = fixture.shortcuts();
        let mut state = HotkeyState::load(&shortcuts, "<Super>space");

        state.set_candidate(&shortcuts, "<Win>space");

        assert!(state.error.as_deref().unwrap().contains("<Win>"));
        assert!(!state.can_set());
        assert_eq!(state.status(str::to_owned), state.error.clone().unwrap());
    }

    #[test]
    fn clear_removes_the_key_of_petlauncher_and_keeps_the_other_bindings() {
        let fixture = SettingsBackendFixture::with_conflicts();
        let shortcuts = fixture.shortcuts();
        shortcuts
            .set_global_key("<Control><Super>k", TOGGLE_COMMAND)
            .unwrap();
        let mut state = HotkeyState::load(&shortcuts, "");

        state.clear(&shortcuts).unwrap();

        assert_eq!(shortcuts.global_key(), None);
        assert_eq!(state.current, None);
        assert_eq!(fixture.binding(NOTES), "<Super>space");
        assert!(state.status(str::to_owned).starts_with("Press a key"));
    }

    #[test]
    fn the_shortcut_of_petlauncher_is_never_a_conflict() {
        let fixture = SettingsBackendFixture::with_conflicts();
        let shortcuts = fixture.shortcuts();
        shortcuts
            .set_global_key("<Control><Super>k", TOGGLE_COMMAND)
            .unwrap();

        let state = HotkeyState::load(&shortcuts, "");

        assert_eq!(state.candidate, "<Control><Super>k");
        assert!(state
            .conflicts
            .iter()
            .all(|conflict| !matches!(&conflict.kind, BindingKind::Custom { path } if path.contains("/petlauncher/"))));
    }

    #[test]
    fn a_removed_conflict_index_that_is_gone_is_an_error() {
        let fixture = SettingsBackendFixture::with_conflicts();
        let shortcuts = fixture.shortcuts();
        let mut state = HotkeyState::load(&shortcuts, "<Super>space");

        assert!(state.remove_conflict(&shortcuts, 5).is_err());
    }

    #[test]
    fn capture_writes_the_modifiers_and_the_key() {
        assert_eq!(
            capture(gdk::Key::space, M::SUPER_MASK),
            Capture::Key("<Super>space".into())
        );
        assert_eq!(
            capture(gdk::Key::K, M::CONTROL_MASK | M::SUPER_MASK | M::SHIFT_MASK),
            Capture::Key("<Control><Shift><Super>k".into())
        );
        assert_eq!(
            capture(gdk::Key::F12, M::empty()),
            Capture::Key("F12".into())
        );
    }

    #[test]
    fn capture_waits_for_a_key_after_a_modifier_and_escape_cancels() {
        assert_eq!(capture(gdk::Key::Super_L, M::empty()), Capture::Wait);
        assert_eq!(capture(gdk::Key::Control_L, M::SUPER_MASK), Capture::Wait);
        assert_eq!(capture(gdk::Key::Escape, M::empty()), Capture::Cancel);
        assert_eq!(
            capture(gdk::Key::Escape, M::SUPER_MASK),
            Capture::Key("<Super>Escape".into())
        );
    }
}
