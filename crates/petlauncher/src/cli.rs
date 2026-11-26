//! The command line of `petlauncher`. `docs/program.md` describes it.

use std::ffi::OsString;
use std::fmt;

pub const USAGE: &str = "\
Usage:
  petlauncher                          Start petlauncher and show the launcher
  petlauncher --hidden                 Start petlauncher with no window (start at login)
  petlauncher toggle                   Show or hide the launcher (the global key)
  petlauncher convert-shortcuts [--force]
                                       Write the Ulauncher shortcuts to shortcuts.toml
  petlauncher --version                Show the version
  petlauncher --help                   Show this text
";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Starts the program, or gives the command to the program that runs.
    /// `hidden`: no window, for the start at login.
    Start {
        hidden: bool,
    },
    /// Shows or hides the launcher. Starts the program when it does not run.
    Toggle,
    ConvertShortcuts {
        force: bool,
    },
    Version,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n\n{USAGE}", self.0)
    }
}

/// Parses the arguments after the program name.
pub fn parse<I>(args: I) -> Result<Command, UsageError>
where
    I: IntoIterator<Item = OsString>,
{
    let args = args
        .into_iter()
        .map(|arg| {
            arg.into_string()
                .map_err(|arg| UsageError(format!("The argument {arg:?} is not UTF-8 text.")))
        })
        .collect::<Result<Vec<String>, UsageError>>()?;
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] => Ok(Command::Start { hidden: false }),
        ["--hidden"] => Ok(Command::Start { hidden: true }),
        ["toggle"] => Ok(Command::Toggle),
        ["convert-shortcuts"] => Ok(Command::ConvertShortcuts { force: false }),
        ["convert-shortcuts", "--force"] => Ok(Command::ConvertShortcuts { force: true }),
        ["--version"] => Ok(Command::Version),
        ["--help" | "-h"] => Ok(Command::Help),
        [first, ..] if args.len() == 1 => Err(UsageError(format!("Unknown argument: {first}"))),
        _ => Err(UsageError(format!("Unknown arguments: {}", args.join(" ")))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_text(args: &[&str]) -> Result<Command, UsageError> {
        parse(args.iter().map(OsString::from))
    }

    #[test]
    fn no_argument_starts_the_program_with_the_launcher() {
        assert_eq!(parse_text(&[]), Ok(Command::Start { hidden: false }));
    }

    #[test]
    fn hidden_starts_the_program_with_no_window() {
        assert_eq!(
            parse_text(&["--hidden"]),
            Ok(Command::Start { hidden: true })
        );
    }

    #[test]
    fn toggle_is_a_command() {
        assert_eq!(parse_text(&["toggle"]), Ok(Command::Toggle));
    }

    #[test]
    fn convert_shortcuts_accepts_force() {
        assert_eq!(
            parse_text(&["convert-shortcuts"]),
            Ok(Command::ConvertShortcuts { force: false })
        );
        assert_eq!(
            parse_text(&["convert-shortcuts", "--force"]),
            Ok(Command::ConvertShortcuts { force: true })
        );
    }

    #[test]
    fn version_and_help_are_options() {
        assert_eq!(parse_text(&["--version"]), Ok(Command::Version));
        assert_eq!(parse_text(&["--help"]), Ok(Command::Help));
        assert_eq!(parse_text(&["-h"]), Ok(Command::Help));
    }

    #[test]
    fn rejects_an_unknown_argument_with_its_name() {
        let error = parse_text(&["show"]).unwrap_err();

        assert_eq!(error.0, "Unknown argument: show");
        assert!(error.to_string().contains("Usage:"));
    }

    #[test]
    fn rejects_extra_arguments() {
        for args in [
            &["toggle", "now"][..],
            &["--hidden", "toggle"],
            &["convert-shortcuts", "--force", "x"],
            &["--force", "convert-shortcuts"],
        ] {
            let error = parse_text(args).unwrap_err();

            assert_eq!(error.0, format!("Unknown arguments: {}", args.join(" ")));
        }
    }

    #[test]
    fn rejects_an_argument_that_is_not_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let args = vec![OsString::from_vec(vec![0xff, 0xfe])];

        let error = parse(args).unwrap_err();

        assert!(error.0.contains("not UTF-8"), "{error}");
    }
}
