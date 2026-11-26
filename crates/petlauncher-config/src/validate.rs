//! Checks of the values that TOML types alone do not catch.
//!
//! An error is a value that petlauncher cannot use in a defined way. A file
//! with an error does not go into effect, and the settings window does not
//! save a change that gives an error. A warning is a value that works but
//! that is probably not what the owner wants. A warning never stops a file
//! or a change. A conflict between the two files is always a warning,
//! because then no single file is wrong.

use std::fmt;
use std::ops::RangeInclusive;

use crate::model::{Settings, Shortcut, WindowPosition};
use crate::paths::ConfigFile;

/// The permitted values of `window_width`, in pixels.
pub const WINDOW_WIDTH: RangeInclusive<u32> = 300..=7680;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}

/// One result of a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub file: ConfigFile,
    /// The line in the file, from 1. Only the functions that read a file
    /// know the line. [`validate`] gives `None`.
    pub line: Option<usize>,
    pub kind: ProblemKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemKind {
    /// Error: `window_width` is not in [`WINDOW_WIDTH`].
    WindowWidthOutOfRange { width: u32 },
    /// Error: `custom_position` has a value that is not from 0 to 1.
    CustomPositionOutOfRange,
    /// Warning: `window_position` is `custom`, but the file has no
    /// `custom_position`. The window opens at the center.
    NoCustomPosition,
    /// Error: a keyword is empty.
    EmptyKeyword { owner: KeywordOwner },
    /// Error: a keyword contains a space, a tab or a line end. The query
    /// cannot start with such a keyword, because a space separates the
    /// keyword from the argument.
    KeywordWithSpace {
        owner: KeywordOwner,
        keyword: String,
    },
    /// Warning: `shortcut` has the same keyword as the earlier `first`.
    DuplicateKeyword {
        keyword: String,
        first: ShortcutRef,
        shortcut: ShortcutRef,
    },
    /// Warning: a shortcut has the keyword of the file search or of the
    /// dictionary.
    KeywordOfBuiltIn {
        keyword: String,
        shortcut: ShortcutRef,
        built_in: BuiltIn,
    },
    /// Warning: the file search and the dictionary have the same keyword.
    SameBuiltInKeyword { keyword: String },
    /// Warning: the file has a key that petlauncher does not know, for
    /// example because of a spelling mistake. The key has no effect.
    UnknownKey { key: String },
}

/// The item that has a keyword.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordOwner {
    BuiltIn(BuiltIn),
    Shortcut(ShortcutRef),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltIn {
    FileSearch,
    Dictionary,
}

/// A shortcut by its position in `shortcuts.toml`, from 0, and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutRef {
    pub index: usize,
    pub name: String,
}

impl ProblemKind {
    pub fn severity(&self) -> Severity {
        match self {
            Self::WindowWidthOutOfRange { .. }
            | Self::CustomPositionOutOfRange
            | Self::EmptyKeyword { .. }
            | Self::KeywordWithSpace { .. } => Severity::Error,
            Self::DuplicateKeyword { .. }
            | Self::KeywordOfBuiltIn { .. }
            | Self::SameBuiltInKeyword { .. }
            | Self::NoCustomPosition
            | Self::UnknownKey { .. } => Severity::Warning,
        }
    }
}

impl Problem {
    pub fn severity(&self) -> Severity {
        self.kind.severity()
    }

    pub fn is_error(&self) -> bool {
        self.severity() == Severity::Error
    }
}

/// All checks of the two files together. The settings window uses this
/// before it saves a change.
pub fn validate(settings: &Settings, shortcuts: &[Shortcut]) -> Vec<Problem> {
    let mut problems = validate_settings(settings);
    problems.extend(validate_shortcuts(shortcuts));
    problems.extend(validate_across(settings, shortcuts));
    problems
}

/// The checks that need only `settings.toml`.
pub fn validate_settings(settings: &Settings) -> Vec<Problem> {
    let mut kinds = Vec::new();
    if !WINDOW_WIDTH.contains(&settings.window_width) {
        kinds.push(ProblemKind::WindowWidthOutOfRange {
            width: settings.window_width,
        });
    }
    match settings.custom_position {
        Some(place)
            if ![place.x, place.y]
                .iter()
                .all(|value| (0.0..=1.0).contains(value)) =>
        {
            kinds.push(ProblemKind::CustomPositionOutOfRange);
        }
        None if settings.window_position == WindowPosition::Custom => {
            kinds.push(ProblemKind::NoCustomPosition);
        }
        _ => {}
    }
    let file_search = &settings.file_search.keyword;
    let dictionary = &settings.dictionary.keyword;
    kinds.extend(keyword_error(
        file_search,
        KeywordOwner::BuiltIn(BuiltIn::FileSearch),
    ));
    kinds.extend(keyword_error(
        dictionary,
        KeywordOwner::BuiltIn(BuiltIn::Dictionary),
    ));
    if is_usable(file_search) && file_search == dictionary {
        kinds.push(ProblemKind::SameBuiltInKeyword {
            keyword: file_search.clone(),
        });
    }
    in_file(ConfigFile::Settings, kinds)
}

/// The checks that need only `shortcuts.toml`.
pub fn validate_shortcuts(shortcuts: &[Shortcut]) -> Vec<Problem> {
    let mut kinds = Vec::new();
    for (index, shortcut) in shortcuts.iter().enumerate() {
        let this = shortcut_ref(index, shortcut);
        kinds.extend(keyword_error(
            &shortcut.keyword,
            KeywordOwner::Shortcut(this.clone()),
        ));
        if !is_usable(&shortcut.keyword) {
            continue;
        }
        let first = shortcuts[..index]
            .iter()
            .position(|earlier| earlier.keyword == shortcut.keyword);
        if let Some(first) = first {
            kinds.push(ProblemKind::DuplicateKeyword {
                keyword: shortcut.keyword.clone(),
                first: shortcut_ref(first, &shortcuts[first]),
                shortcut: this,
            });
        }
    }
    in_file(ConfigFile::Shortcuts, kinds)
}

/// The checks that compare the keywords of the two files.
pub(crate) fn validate_across(settings: &Settings, shortcuts: &[Shortcut]) -> Vec<Problem> {
    let built_ins = [
        (BuiltIn::FileSearch, &settings.file_search.keyword),
        (BuiltIn::Dictionary, &settings.dictionary.keyword),
    ];
    let mut kinds = Vec::new();
    for (index, shortcut) in shortcuts.iter().enumerate() {
        let conflict = built_ins
            .iter()
            .find(|(_, keyword)| is_usable(keyword) && **keyword == shortcut.keyword);
        if let Some((built_in, keyword)) = conflict {
            kinds.push(ProblemKind::KeywordOfBuiltIn {
                keyword: (*keyword).clone(),
                shortcut: shortcut_ref(index, shortcut),
                built_in: *built_in,
            });
        }
    }
    in_file(ConfigFile::Shortcuts, kinds)
}

fn keyword_error(keyword: &str, owner: KeywordOwner) -> Option<ProblemKind> {
    if keyword.trim().is_empty() {
        Some(ProblemKind::EmptyKeyword { owner })
    } else if keyword.chars().any(char::is_whitespace) {
        Some(ProblemKind::KeywordWithSpace {
            owner,
            keyword: keyword.to_owned(),
        })
    } else {
        None
    }
}

fn is_usable(keyword: &str) -> bool {
    !keyword.is_empty() && !keyword.chars().any(char::is_whitespace)
}

fn shortcut_ref(index: usize, shortcut: &Shortcut) -> ShortcutRef {
    ShortcutRef {
        index,
        name: shortcut.name.clone(),
    }
}

fn in_file(file: ConfigFile, kinds: Vec<ProblemKind>) -> Vec<Problem> {
    kinds
        .into_iter()
        .map(|kind| Problem {
            file,
            line: None,
            kind,
        })
        .collect()
}

impl fmt::Display for ShortcutRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "shortcut {} \"{}\"", self.index + 1, self.name)
    }
}

impl fmt::Display for BuiltIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FileSearch => "the file search",
            Self::Dictionary => "the dictionary",
        })
    }
}

impl fmt::Display for KeywordOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuiltIn(built_in) => built_in.fmt(f),
            Self::Shortcut(shortcut) => shortcut.fmt(f),
        }
    }
}

impl fmt::Display for ProblemKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WindowWidthOutOfRange { width } => write!(
                f,
                "window_width {width} is not from {} to {}",
                WINDOW_WIDTH.start(),
                WINDOW_WIDTH.end()
            ),
            Self::CustomPositionOutOfRange => {
                write!(f, "custom_position has a value that is not from 0 to 1")
            }
            Self::NoCustomPosition => write!(
                f,
                "window_position is \"custom\", but there is no custom_position, so the window \
                 opens at the center"
            ),
            Self::EmptyKeyword { owner } => write!(f, "the keyword of {owner} is empty"),
            Self::KeywordWithSpace { owner, keyword } => {
                write!(f, "the keyword \"{keyword}\" of {owner} contains a space")
            }
            Self::DuplicateKeyword {
                keyword,
                first,
                shortcut,
            } => write!(f, "{shortcut} has the keyword \"{keyword}\" of {first}"),
            Self::KeywordOfBuiltIn {
                keyword,
                shortcut,
                built_in,
            } => write!(f, "{shortcut} has the keyword \"{keyword}\" of {built_in}"),
            Self::SameBuiltInKeyword { keyword } => write!(
                f,
                "the file search and the dictionary have the same keyword \"{keyword}\""
            ),
            Self::UnknownKey { key } => write!(f, "unknown key \"{key}\" has no effect"),
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{line}: {}", self.file, self.kind),
            None => write!(f, "{}: {}", self.file, self.kind),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut(name: &str, keyword: &str) -> Shortcut {
        Shortcut {
            name: name.to_owned(),
            keyword: keyword.to_owned(),
            cmd: "https://example.com/?q=%s".to_owned(),
            ..Shortcut::default()
        }
    }

    fn kinds(problems: Vec<Problem>) -> Vec<ProblemKind> {
        problems.into_iter().map(|problem| problem.kind).collect()
    }

    #[test]
    fn finds_no_problem_in_the_initial_settings() {
        let problems = validate(&Settings::default(), &[shortcut("Jira", "jira")]);

        assert_eq!(problems, vec![]);
    }

    #[test]
    fn gives_an_error_for_a_window_width_out_of_range() {
        for width in [0, 299, 7681] {
            let settings = Settings {
                window_width: width,
                ..Settings::default()
            };

            let problems = validate_settings(&settings);

            assert_eq!(
                kinds(problems.clone()),
                vec![ProblemKind::WindowWidthOutOfRange { width }]
            );
            assert!(problems[0].is_error());
            assert_eq!(problems[0].file, ConfigFile::Settings);
        }
    }

    #[test]
    fn accepts_the_limits_of_the_window_width() {
        for width in [*WINDOW_WIDTH.start(), *WINDOW_WIDTH.end()] {
            let settings = Settings {
                window_width: width,
                ..Settings::default()
            };

            assert_eq!(validate_settings(&settings), vec![]);
        }
    }

    #[test]
    fn gives_an_error_for_an_empty_built_in_keyword() {
        let mut settings = Settings::default();
        settings.file_search.keyword = String::new();
        settings.dictionary.keyword = " ".to_owned();

        let problems = validate_settings(&settings);

        assert_eq!(
            kinds(problems.clone()),
            vec![
                ProblemKind::EmptyKeyword {
                    owner: KeywordOwner::BuiltIn(BuiltIn::FileSearch)
                },
                ProblemKind::EmptyKeyword {
                    owner: KeywordOwner::BuiltIn(BuiltIn::Dictionary)
                },
            ]
        );
        assert!(problems.iter().all(Problem::is_error));
    }

    #[test]
    fn gives_a_warning_when_the_file_search_and_the_dictionary_share_a_keyword() {
        let mut settings = Settings::default();
        settings.dictionary.keyword = "f".to_owned();

        let problems = validate_settings(&settings);

        assert_eq!(
            kinds(problems.clone()),
            vec![ProblemKind::SameBuiltInKeyword {
                keyword: "f".to_owned()
            }]
        );
        assert_eq!(problems[0].severity(), Severity::Warning);
    }

    #[test]
    fn gives_an_error_for_an_empty_shortcut_keyword() {
        let problems = validate_shortcuts(&[shortcut("Jira", "jira"), shortcut("Wiki", "")]);

        assert_eq!(
            kinds(problems.clone()),
            vec![ProblemKind::EmptyKeyword {
                owner: KeywordOwner::Shortcut(ShortcutRef {
                    index: 1,
                    name: "Wiki".to_owned()
                })
            }]
        );
        assert!(problems[0].is_error());
        assert_eq!(problems[0].file, ConfigFile::Shortcuts);
    }

    #[test]
    fn gives_an_error_for_a_keyword_with_a_space() {
        let problems = validate_shortcuts(&[shortcut("Wiki", "my wiki")]);

        assert_eq!(
            kinds(problems),
            vec![ProblemKind::KeywordWithSpace {
                owner: KeywordOwner::Shortcut(ShortcutRef {
                    index: 0,
                    name: "Wiki".to_owned()
                }),
                keyword: "my wiki".to_owned(),
            }]
        );
    }

    #[test]
    fn gives_a_warning_for_each_repeated_shortcut_keyword() {
        let shortcuts = [
            shortcut("A", "x"),
            shortcut("B", "y"),
            shortcut("C", "x"),
            shortcut("D", "x"),
        ];

        let problems = validate_shortcuts(&shortcuts);

        let first = ShortcutRef {
            index: 0,
            name: "A".to_owned(),
        };
        assert_eq!(
            kinds(problems.clone()),
            vec![
                ProblemKind::DuplicateKeyword {
                    keyword: "x".to_owned(),
                    first: first.clone(),
                    shortcut: ShortcutRef {
                        index: 2,
                        name: "C".to_owned()
                    },
                },
                ProblemKind::DuplicateKeyword {
                    keyword: "x".to_owned(),
                    first,
                    shortcut: ShortcutRef {
                        index: 3,
                        name: "D".to_owned()
                    },
                },
            ]
        );
        assert!(problems.iter().all(|problem| !problem.is_error()));
    }

    #[test]
    fn gives_a_warning_when_a_shortcut_has_a_built_in_keyword() {
        let shortcuts = [shortcut("Files", "f"), shortcut("Cambridge", "d")];

        let problems = validate(&Settings::default(), &shortcuts);

        assert_eq!(
            kinds(problems.clone()),
            vec![
                ProblemKind::KeywordOfBuiltIn {
                    keyword: "f".to_owned(),
                    shortcut: ShortcutRef {
                        index: 0,
                        name: "Files".to_owned()
                    },
                    built_in: BuiltIn::FileSearch,
                },
                ProblemKind::KeywordOfBuiltIn {
                    keyword: "d".to_owned(),
                    shortcut: ShortcutRef {
                        index: 1,
                        name: "Cambridge".to_owned()
                    },
                    built_in: BuiltIn::Dictionary,
                },
            ]
        );
        assert!(problems
            .iter()
            .all(|problem| problem.file == ConfigFile::Shortcuts));
        assert!(problems.iter().all(|problem| !problem.is_error()));
    }

    #[test]
    fn does_not_report_an_empty_keyword_as_a_duplicate_or_a_conflict() {
        let mut settings = Settings::default();
        settings.dictionary.keyword = String::new();

        let problems = validate(&settings, &[shortcut("A", ""), shortcut("B", "")]);

        assert!(problems
            .iter()
            .all(|problem| matches!(problem.kind, ProblemKind::EmptyKeyword { .. })));
        assert_eq!(problems.len(), 3);
    }

    #[test]
    fn writes_a_message_that_names_the_shortcut_and_the_line() {
        let problem = Problem {
            file: ConfigFile::Shortcuts,
            line: Some(12),
            kind: ProblemKind::DuplicateKeyword {
                keyword: "x".to_owned(),
                first: ShortcutRef {
                    index: 0,
                    name: "A".to_owned(),
                },
                shortcut: ShortcutRef {
                    index: 2,
                    name: "C".to_owned(),
                },
            },
        };

        assert_eq!(
            problem.to_string(),
            "shortcuts.toml:12: shortcut 3 \"C\" has the keyword \"x\" of shortcut 1 \"A\""
        );
    }

    #[test]
    fn writes_a_message_for_each_kind() {
        let wiki = ShortcutRef {
            index: 0,
            name: "Wiki".to_owned(),
        };
        let cases = [
            (
                ProblemKind::WindowWidthOutOfRange { width: 10 },
                "window_width 10 is not from 300 to 7680",
            ),
            (
                ProblemKind::EmptyKeyword {
                    owner: KeywordOwner::BuiltIn(BuiltIn::FileSearch),
                },
                "the keyword of the file search is empty",
            ),
            (
                ProblemKind::KeywordWithSpace {
                    owner: KeywordOwner::Shortcut(wiki.clone()),
                    keyword: "my wiki".to_owned(),
                },
                "the keyword \"my wiki\" of shortcut 1 \"Wiki\" contains a space",
            ),
            (
                ProblemKind::KeywordOfBuiltIn {
                    keyword: "d".to_owned(),
                    shortcut: wiki,
                    built_in: BuiltIn::Dictionary,
                },
                "shortcut 1 \"Wiki\" has the keyword \"d\" of the dictionary",
            ),
            (
                ProblemKind::SameBuiltInKeyword {
                    keyword: "f".to_owned(),
                },
                "the file search and the dictionary have the same keyword \"f\"",
            ),
            (
                ProblemKind::UnknownKey {
                    key: "file_search.ignores".to_owned(),
                },
                "unknown key \"file_search.ignores\" has no effect",
            ),
        ];

        for (kind, message) in cases {
            assert_eq!(kind.to_string(), message);
        }
        let without_line = Problem {
            file: ConfigFile::Settings,
            line: None,
            kind: ProblemKind::WindowWidthOutOfRange { width: 10 },
        };
        assert_eq!(
            without_line.to_string(),
            "settings.toml: window_width 10 is not from 300 to 7680"
        );
    }
}
