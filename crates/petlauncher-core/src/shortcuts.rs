//! The rows of the shortcuts (R6, R7, R12).

use std::path::Path;

use petlauncher_config::Shortcut;

use crate::contract::{Action, Icon, Row, RowKind};

/// True when the typed keyword is the keyword of a setting or a shortcut.
/// The comparison ignores letter case. An empty keyword matches nothing.
pub(crate) fn same_keyword(typed: &str, keyword: &str) -> bool {
    !keyword.is_empty() && typed.to_lowercase() == keyword.to_lowercase()
}

/// One row for each shortcut that has the keyword.
pub(crate) fn keyword_rows(
    shortcuts: &[Shortcut],
    keyword: &str,
    argument: &str,
    home: &Path,
) -> Vec<Row> {
    shortcuts
        .iter()
        .filter(|shortcut| same_keyword(keyword, &shortcut.keyword))
        .map(|shortcut| shortcut_row(shortcut, argument, home))
        .collect()
}

/// The fallback rows of R6: each shortcut with `is_default_search`, with
/// the full query as the argument.
pub(crate) fn fallback_rows(shortcuts: &[Shortcut], query: &str, home: &Path) -> Vec<Row> {
    shortcuts
        .iter()
        .filter(|shortcut| shortcut.is_default_search)
        .map(|shortcut| shortcut_row(shortcut, query, home))
        .collect()
}

fn shortcut_row(shortcut: &Shortcut, argument: &str, home: &Path) -> Row {
    let enter = shortcut_action(shortcut, argument);
    let subtitle = match &enter {
        Action::OpenUrl { url, .. } => url.clone(),
        Action::RunScript { argument, .. } if argument.is_empty() => "Run the script".to_owned(),
        Action::RunScript { argument, .. } => format!("Run the script with \"{argument}\""),
        _ => format!("Type an argument after \"{} \"", shortcut.keyword),
    };
    let title = if shortcut.name.is_empty() {
        &shortcut.keyword
    } else {
        &shortcut.name
    };
    Row::new(RowKind::Shortcut, title.clone(), enter)
        .with_subtitle(subtitle)
        .with_icon(shortcut_icon(&shortcut.icon, home))
}

/// The target of the shortcut. With an argument: `cmd`. With no argument:
/// `cmd_without_argument` (R7), else `cmd` when `run_without_argument` is
/// on, else nothing (R6).
fn shortcut_action(shortcut: &Shortcut, argument: &str) -> Action {
    let browser = shortcut.browser.as_ref();
    if !argument.is_empty() {
        target(&shortcut.cmd, argument, browser)
    } else if !shortcut.cmd_without_argument.trim().is_empty() {
        target(&shortcut.cmd_without_argument, "", browser)
    } else if shortcut.run_without_argument {
        target(&shortcut.cmd, "", browser)
    } else {
        Action::Nothing
    }
}

/// A command that starts with `http://` or `https://` is a URL. Other text
/// is a script.
fn target(command: &str, argument: &str, browser: Option<&petlauncher_config::Browser>) -> Action {
    let command = command.trim();
    if is_url(command) {
        Action::OpenUrl {
            url: command.replace("%s", &percent_encode(argument)),
            browser: browser.cloned(),
        }
    } else {
        Action::RunScript {
            script: command.replace("%s", argument),
            argument: argument.to_owned(),
        }
    }
}

fn is_url(command: &str) -> bool {
    let lower = command.get(..8).unwrap_or(command).to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Encodes each byte except the letters, the digits and `-._~/`, so that a
/// space, `&` or `#` in the argument does not change the URL.
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// An icon text that starts with `/` or `~/` is a file. Other text is a
/// name in the icon theme.
fn shortcut_icon(icon: &str, home: &Path) -> Icon {
    let icon = icon.trim();
    if icon.is_empty() {
        Icon::None
    } else if let Some(rest) = icon.strip_prefix("~/") {
        Icon::Path(home.join(rest))
    } else if icon.starts_with('/') {
        Icon::Path(icon.into())
    } else {
        Icon::Name(icon.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petlauncher_config::Browser;

    const HOME: &str = "/home/owner";

    fn shortcut(keyword: &str, cmd: &str) -> Shortcut {
        Shortcut {
            name: format!("{keyword} name"),
            keyword: keyword.into(),
            cmd: cmd.into(),
            ..Shortcut::default()
        }
    }

    fn enter(shortcut: &Shortcut, argument: &str) -> Action {
        shortcut_row(shortcut, argument, Path::new(HOME)).enter
    }

    fn url(url: &str) -> Action {
        Action::OpenUrl {
            url: url.into(),
            browser: None,
        }
    }

    #[test]
    fn r6_percent_s_in_a_url_becomes_the_argument() {
        let jira = shortcut("jira", "https://jira.example.com/browse/%s");

        assert_eq!(
            enter(&jira, "ABC-12"),
            url("https://jira.example.com/browse/ABC-12")
        );
    }

    #[test]
    fn r6_the_argument_in_a_url_is_percent_encoded() {
        let google = shortcut("g", "https://www.google.com/search?q=%s");

        assert_eq!(
            enter(&google, "rust & go #1 ä"),
            url("https://www.google.com/search?q=rust%20%26%20go%20%231%20%C3%A4")
        );
    }

    #[test]
    fn r6_a_command_that_is_not_a_url_is_a_script_with_the_argument() {
        let script = shortcut(
            "beacon",
            "#!/bin/sh\nmicrosoft-edge \"https://b.example/%s\" $1",
        );

        assert_eq!(
            enter(&script, "a b"),
            Action::RunScript {
                script: "#!/bin/sh\nmicrosoft-edge \"https://b.example/a b\" $1".into(),
                argument: "a b".into(),
            }
        );
    }

    #[test]
    fn r6_a_url_scheme_in_capital_letters_is_a_url() {
        let upper = shortcut("u", "HTTPS://example.com/%s");

        assert_eq!(enter(&upper, "x"), url("HTTPS://example.com/x"));
    }

    #[test]
    fn r6_enter_with_no_argument_does_nothing_when_the_shortcut_needs_one() {
        let google = shortcut("g", "https://www.google.com/search?q=%s");

        let row = shortcut_row(&google, "", Path::new(HOME));

        assert_eq!(row.enter, Action::Nothing);
        assert_eq!(row.subtitle, "Type an argument after \"g \"");
    }

    #[test]
    fn r6_run_without_argument_runs_the_command_with_an_empty_argument() {
        let ziba = Shortcut {
            run_without_argument: true,
            ..shortcut("ziba", "https://ziba.example.com/%s")
        };

        assert_eq!(enter(&ziba, ""), url("https://ziba.example.com/"));
    }

    #[test]
    fn r7_command_without_argument_wins_over_run_without_argument() {
        let rektangel = Shortcut {
            cmd_without_argument: "https://github.com/LOLm/cat/pulls".into(),
            run_without_argument: true,
            ..shortcut("rektangel", "https://github.com/LOLm/cat/pull/%s")
        };

        assert_eq!(
            enter(&rektangel, ""),
            url("https://github.com/LOLm/cat/pulls")
        );
    }

    #[test]
    fn r7_a_script_without_argument_gets_an_empty_argument() {
        let script = Shortcut {
            cmd_without_argument: "notify-send 'no %s'".into(),
            ..shortcut("n", "notify-send %s")
        };

        assert_eq!(
            enter(&script, ""),
            Action::RunScript {
                script: "notify-send 'no '".into(),
                argument: String::new(),
            }
        );
    }

    #[test]
    fn r12_the_url_opens_in_the_browser_of_the_shortcut() {
        let edge = Browser::App("microsoft-edge.desktop".into());
        let beacon = Shortcut {
            browser: Some(edge.clone()),
            ..shortcut("beacon", "https://beacon.example.com/%s")
        };

        assert_eq!(
            enter(&beacon, "7"),
            Action::OpenUrl {
                url: "https://beacon.example.com/7".into(),
                browser: Some(edge),
            }
        );
    }

    #[test]
    fn the_row_shows_the_name_the_url_and_the_icon() {
        let jira = Shortcut {
            icon: "~/icons/jira.png".into(),
            ..shortcut("jira", "https://jira.example.com/browse/%s")
        };

        let row = shortcut_row(&jira, "A-1", Path::new(HOME));

        assert_eq!(row.title, "jira name");
        assert_eq!(row.subtitle, "https://jira.example.com/browse/A-1");
        assert_eq!(row.icon, Icon::Path("/home/owner/icons/jira.png".into()));
        assert_eq!(row.kind, RowKind::Shortcut);
        assert_eq!(row.local_file, None);
    }

    #[test]
    fn a_shortcut_with_no_name_shows_its_keyword() {
        let mut bare = shortcut("bare", "https://example.com/%s");
        bare.name.clear();

        assert_eq!(shortcut_row(&bare, "", Path::new(HOME)).title, "bare");
    }

    #[test]
    fn a_script_row_tells_that_it_runs_a_script() {
        let script = Shortcut {
            run_without_argument: true,
            ..shortcut("s", "echo %s")
        };

        assert_eq!(
            shortcut_row(&script, "", Path::new(HOME)).subtitle,
            "Run the script"
        );
        assert_eq!(
            shortcut_row(&script, "x", Path::new(HOME)).subtitle,
            "Run the script with \"x\""
        );
    }

    #[test]
    fn the_icon_text_is_a_file_or_a_theme_name() {
        let home = Path::new(HOME);

        assert_eq!(shortcut_icon("", home), Icon::None);
        assert_eq!(
            shortcut_icon("/a/b.svg", home),
            Icon::Path("/a/b.svg".into())
        );
        assert_eq!(
            shortcut_icon("web-browser", home),
            Icon::Name("web-browser".into())
        );
    }

    #[test]
    fn the_keyword_match_ignores_letter_case_and_an_empty_keyword_matches_nothing() {
        assert!(same_keyword("Jira", "jira"));
        assert!(!same_keyword("jir", "jira"));
        assert!(!same_keyword("", ""));
    }
}
