//! The built-in providers of R16: system commands and settings pages.

use std::path::PathBuf;

use petlauncher_config::Settings;

use crate::cancel::CancelToken;
use crate::contract::{Action, Icon, Row, RowKind, SystemCommand};
use crate::matching::match_rank;
use crate::provider::Provider;

/// The rows Lock, Suspend, Log out, Restart and Power off. `Engine::new`
/// adds this provider.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemCommands;

impl Provider for SystemCommands {
    fn search(&self, text: &str, _settings: &Settings, _cancel: &CancelToken) -> Vec<Row> {
        let mut matches: Vec<(u8, SystemCommand)> = SystemCommand::ALL
            .into_iter()
            .filter_map(|command| {
                match_rank(text, command.title(), keywords(command)).map(|rank| (rank, command))
            })
            .collect();
        matches.sort_by_key(|(rank, _)| *rank);
        matches
            .into_iter()
            .map(|(_, command)| system_row(command))
            .collect()
    }
}

fn keywords(command: SystemCommand) -> &'static [&'static str] {
    match command {
        SystemCommand::Lock => &["lock screen"],
        SystemCommand::Suspend => &["sleep"],
        SystemCommand::LogOut => &["logout", "sign out"],
        SystemCommand::Restart => &["reboot"],
        SystemCommand::PowerOff => &["shutdown", "shut down"],
    }
}

fn system_row(command: SystemCommand) -> Row {
    let (subtitle, icon) = match command {
        SystemCommand::Lock => ("Lock the screen", "system-lock-screen"),
        SystemCommand::Suspend => ("Put the computer in the suspend state", "system-suspend"),
        SystemCommand::LogOut => ("Log out of the session", "system-log-out"),
        SystemCommand::Restart => ("Restart the computer", "system-reboot"),
        SystemCommand::PowerOff => ("Power off the computer", "system-shutdown"),
    };
    Row::new(
        RowKind::SystemCommand,
        command.title(),
        Action::System(command),
    )
    .with_subtitle(subtitle)
    .with_icon(Icon::Name(icon.to_owned()))
}

/// One page of the system settings, for example Wi-Fi (R16).
/// `Desktop::settings_pages` gives the list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsPage {
    pub title: String,
    pub keywords: Vec<String>,
    pub icon: Icon,
    /// The `.desktop` file that opens the page. Enter starts it, and it is
    /// the local file of the row (R2, R13).
    pub desktop_file: PathBuf,
}

/// The rows of the settings pages.
#[derive(Debug, Clone, Default)]
pub struct SettingsPages {
    pages: Vec<SettingsPage>,
}

impl SettingsPages {
    pub fn new(pages: Vec<SettingsPage>) -> Self {
        Self { pages }
    }
}

impl Provider for SettingsPages {
    fn search(&self, text: &str, _settings: &Settings, _cancel: &CancelToken) -> Vec<Row> {
        let mut matches: Vec<(u8, &SettingsPage)> = self
            .pages
            .iter()
            .filter_map(|page| {
                match_rank(text, &page.title, &page.keywords).map(|rank| (rank, page))
            })
            .collect();
        matches.sort_by(|(rank_a, page_a), (rank_b, page_b)| {
            rank_a
                .cmp(rank_b)
                .then_with(|| page_a.title.cmp(&page_b.title))
        });
        matches
            .into_iter()
            .map(|(_, page)| {
                Row::new(
                    RowKind::SettingsPage,
                    page.title.clone(),
                    Action::LaunchDesktopEntry(page.desktop_file.clone()),
                )
                .with_subtitle("Settings")
                .with_icon(page.icon.clone())
                .with_local_file(page.desktop_file.clone())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(provider: &dyn Provider, text: &str) -> Vec<Row> {
        provider.search(text, &Settings::default(), &CancelToken::new())
    }

    fn titles(rows: &[Row]) -> Vec<&str> {
        rows.iter().map(|row| row.title.as_str()).collect()
    }

    #[test]
    fn system_commands_match_their_titles_and_other_names() {
        assert_eq!(titles(&search(&SystemCommands, "lock")), ["Lock"]);
        assert_eq!(titles(&search(&SystemCommands, "reboot")), ["Restart"]);
        assert_eq!(titles(&search(&SystemCommands, "shutdown")), ["Power off"]);
        assert_eq!(titles(&search(&SystemCommands, "sleep")), ["Suspend"]);
    }

    #[test]
    fn a_system_command_row_gives_the_system_action() {
        let rows = search(&SystemCommands, "log out");

        assert_eq!(rows[0].enter, Action::System(SystemCommand::LogOut));
        assert_eq!(rows[0].kind, RowKind::SystemCommand);
        assert_eq!(rows[0].local_file, None);
    }

    #[test]
    fn a_system_command_with_a_title_match_comes_before_keyword_matches() {
        let rows = search(&SystemCommands, "s");

        assert_eq!(titles(&rows), ["Suspend", "Lock", "Log out", "Power off"]);
    }

    #[test]
    fn a_settings_page_row_starts_its_desktop_entry() {
        let pages = SettingsPages::new(vec![
            SettingsPage {
                title: "Wi-Fi".into(),
                keywords: vec!["wireless".into()],
                icon: Icon::Name("network-wireless".into()),
                desktop_file: "/usr/share/applications/gnome-wifi-panel.desktop".into(),
            },
            SettingsPage {
                title: "Sound".into(),
                keywords: Vec::new(),
                icon: Icon::None,
                desktop_file: "/usr/share/applications/gnome-sound-panel.desktop".into(),
            },
        ]);

        let rows = search(&pages, "wirel");

        assert_eq!(titles(&rows), ["Wi-Fi"]);
        let desktop_file = PathBuf::from("/usr/share/applications/gnome-wifi-panel.desktop");
        assert_eq!(
            rows[0].enter,
            Action::LaunchDesktopEntry(desktop_file.clone())
        );
        assert_eq!(rows[0].local_file, Some(desktop_file));
        assert_eq!(rows[0].kind, RowKind::SettingsPage);
    }
}
