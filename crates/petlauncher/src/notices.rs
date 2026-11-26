//! The notices of the launcher window, below the query line.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeKind {
    /// The last action failed.
    Error,
    /// A state that the owner can correct.
    Warning,
}

/// A line below the query line.
#[derive(Debug, PartialEq, Eq)]
pub struct Notice<'a> {
    pub kind: NoticeKind,
    pub text: &'a str,
    /// The line has a button that opens the Global Key page of the settings
    /// window.
    pub opens_key_settings: bool,
}

/// The texts below the query line: the error of the last action, the
/// global key that is not set (R4), and the error of a settings file (R18).
#[derive(Debug, Default)]
pub struct Notices {
    action: Option<String>,
    global_key: Option<String>,
    config: Option<String>,
}

impl Notices {
    /// The error of the last action. The window stays open with it. A new
    /// text in the query line and a new show remove it.
    pub fn set_action(&mut self, text: Option<String>) {
        self.action = text;
    }

    /// Removes the error of the action. Gives true when there was one.
    pub fn clear_action(&mut self) -> bool {
        self.action.take().is_some()
    }

    pub fn set_global_key(&mut self, text: Option<String>) {
        self.global_key = text;
    }

    pub fn set_config(&mut self, text: Option<String>) {
        self.config = text;
    }

    /// The lines to show, the error of the action first.
    pub fn lines(&self) -> Vec<Notice<'_>> {
        [
            (NoticeKind::Error, &self.action, false),
            (NoticeKind::Warning, &self.global_key, true),
            (NoticeKind::Warning, &self.config, false),
        ]
        .into_iter()
        .filter_map(|(kind, text, opens_key_settings)| {
            text.as_deref().map(|text| Notice {
                kind,
                text,
                opens_key_settings,
            })
        })
        .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_notice_at_the_start() {
        assert!(Notices::default().lines().is_empty());
    }

    #[test]
    fn the_error_of_the_action_comes_first() {
        let mut notices = Notices::default();
        notices.set_config(Some("settings.toml has an error".into()));
        notices.set_global_key(Some("no global key".into()));
        notices.set_action(Some("Could not open".into()));

        let lines = notices.lines();

        assert_eq!(
            lines,
            [
                Notice {
                    kind: NoticeKind::Error,
                    text: "Could not open",
                    opens_key_settings: false,
                },
                Notice {
                    kind: NoticeKind::Warning,
                    text: "no global key",
                    opens_key_settings: true,
                },
                Notice {
                    kind: NoticeKind::Warning,
                    text: "settings.toml has an error",
                    opens_key_settings: false,
                },
            ]
        );
    }

    #[test]
    fn clear_action_tells_if_an_error_was_there() {
        let mut notices = Notices::default();
        notices.set_action(Some("Could not open".into()));

        assert!(notices.clear_action());
        assert!(!notices.clear_action());
    }

    #[test]
    fn a_notice_goes_when_its_text_is_none() {
        let mut notices = Notices::default();
        notices.set_action(Some("Could not open".into()));

        notices.set_action(None);

        assert!(notices.lines().is_empty());
    }
}
