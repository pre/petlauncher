use std::fmt;
use std::path::{Path, PathBuf};

use crate::validate::Problem;

/// The cause that a file does not go into effect, or that petlauncher
/// cannot write it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    /// The file cannot be read or written.
    Io { path: PathBuf, message: String },
    /// The file is not valid TOML, or a value has the wrong type. `line`
    /// and `column` start at 1.
    Toml {
        path: PathBuf,
        line: Option<usize>,
        column: Option<usize>,
        message: String,
    },
    /// The file is valid TOML, but values in it are not permitted. Each
    /// problem is an error, see [`mod@crate::validate`].
    Invalid {
        path: PathBuf,
        problems: Vec<Problem>,
    },
}

impl ConfigError {
    pub fn path(&self) -> &Path {
        match self {
            Self::Io { path, .. } | Self::Toml { path, .. } | Self::Invalid { path, .. } => path,
        }
    }

    /// The line of the first error, from 1, when it is known.
    pub fn line(&self) -> Option<usize> {
        match self {
            Self::Io { .. } => None,
            Self::Toml { line, .. } => *line,
            Self::Invalid { problems, .. } => problems.first().and_then(|problem| problem.line),
        }
    }

    pub(crate) fn io(path: &Path, error: &std::io::Error) -> Self {
        Self::Io {
            path: path.to_owned(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Toml {
                path,
                line,
                column,
                message,
            } => {
                write!(f, "{}", path.display())?;
                if let Some(line) = line {
                    write!(f, ":{line}")?;
                    if let Some(column) = column {
                        write!(f, ":{column}")?;
                    }
                }
                write!(f, ": {message}")
            }
            Self::Invalid { path, problems } => {
                for (number, problem) in problems.iter().enumerate() {
                    if number > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{}", path.display())?;
                    if let Some(line) = problem.line {
                        write!(f, ":{line}")?;
                    }
                    write!(f, ": {}", problem.kind)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// The cause that a change of a file was not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The file cannot be read, parsed or written. Correct the file first.
    File(ConfigError),
    /// The change gives these errors. petlauncher did not write the file.
    Invalid(Vec<Problem>),
    /// There is no shortcut at `index`. The file has `count` shortcuts.
    NoShortcut { index: usize, count: usize },
    /// `shortcuts.toml` changed on the disk after the last reload, so a
    /// shortcut number can refer to a different shortcut. The store read
    /// the file again. Show the new list and do the change again.
    FileChanged,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File(error) => error.fmt(f),
            Self::Invalid(problems) => {
                let messages: Vec<String> = problems.iter().map(|p| p.kind.to_string()).collect();
                f.write_str(&messages.join("; "))
            }
            Self::NoShortcut { index, count } => write!(
                f,
                "there is no shortcut {}, the file has {count} shortcuts",
                index + 1
            ),
            Self::FileChanged => f.write_str("shortcuts.toml changed on the disk, try again"),
        }
    }
}

impl std::error::Error for EditError {}

impl From<ConfigError> for EditError {
    fn from(error: ConfigError) -> Self {
        Self::File(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::ConfigFile;
    use crate::validate::ProblemKind;

    #[test]
    fn writes_the_path_line_and_column_of_a_toml_error() {
        let error = ConfigError::Toml {
            path: PathBuf::from("/c/settings.toml"),
            line: Some(3),
            column: Some(16),
            message: "invalid type".to_owned(),
        };

        assert_eq!(error.to_string(), "/c/settings.toml:3:16: invalid type");
        assert_eq!(error.line(), Some(3));
        assert_eq!(error.path(), Path::new("/c/settings.toml"));
    }

    #[test]
    fn writes_one_line_for_each_problem_of_an_invalid_file() {
        let problem = |line, width| Problem {
            file: ConfigFile::Settings,
            line,
            kind: ProblemKind::WindowWidthOutOfRange { width },
        };
        let error = ConfigError::Invalid {
            path: PathBuf::from("/c/settings.toml"),
            problems: vec![problem(Some(4), 10), problem(None, 20)],
        };

        assert_eq!(
            error.to_string(),
            "/c/settings.toml:4: window_width 10 is not from 300 to 7680\n\
             /c/settings.toml: window_width 20 is not from 300 to 7680"
        );
        assert_eq!(error.line(), Some(4));
    }

    #[test]
    fn writes_an_io_error_and_edit_errors() {
        let io = ConfigError::Io {
            path: PathBuf::from("/c/shortcuts.toml"),
            message: "permission denied".to_owned(),
        };

        assert_eq!(io.to_string(), "/c/shortcuts.toml: permission denied");
        assert_eq!(io.line(), None);
        assert_eq!(EditError::from(io.clone()).to_string(), io.to_string());
        assert_eq!(
            EditError::NoShortcut { index: 4, count: 2 }.to_string(),
            "there is no shortcut 5, the file has 2 shortcuts"
        );
        assert_eq!(
            EditError::Invalid(vec![Problem {
                file: ConfigFile::Settings,
                line: None,
                kind: ProblemKind::WindowWidthOutOfRange { width: 1 },
            }])
            .to_string(),
            "window_width 1 is not from 300 to 7680"
        );
        assert!(EditError::FileChanged.to_string().contains("changed"));
    }
}
