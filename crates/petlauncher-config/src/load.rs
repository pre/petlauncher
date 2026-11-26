//! Read `settings.toml` and `shortcuts.toml`.

use std::io::ErrorKind;
use std::ops::Range;
use std::path::Path;

use serde::{Deserialize, Serialize};
use toml_edit::{Document, Item, TableLike};

use crate::error::ConfigError;
use crate::model::{Settings, Shortcut};
use crate::paths::ConfigFile;
use crate::validate::{
    validate_settings, validate_shortcuts, BuiltIn, KeywordOwner, Problem, ProblemKind,
};

const SETTINGS_KEYS: &[&str] = &[
    "close_on_focus_loss",
    "run_in_background",
    "show_tray_icon",
    "include_applications",
    "hotkey",
    "window_width",
    "window_position",
    "custom_position",
    "keep_query",
    "file_search",
    "dictionary",
];
const FILE_SEARCH_KEYS: &[&str] = &["keyword", "directories", "ignore"];
const DICTIONARY_KEYS: &[&str] = &["keyword", "wordnet_path"];
const CUSTOM_POSITION_KEYS: &[&str] = &["x", "y"];
const SHORTCUTS_KEYS: &[&str] = &["shortcut"];
const SHORTCUT_KEYS: &[&str] = &[
    "name",
    "keyword",
    "cmd",
    "cmd_without_argument",
    "icon",
    "is_default_search",
    "run_without_argument",
    "browser",
];

/// The contents of `shortcuts.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ShortcutsFile {
    pub shortcut: Vec<Shortcut>,
}

/// A file that is in effect, with its warnings.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Loaded<T> {
    pub value: T,
    pub warnings: Vec<Problem>,
}

/// Reads `settings.toml` in `dir`. A missing file gives the initial
/// values. An error in the file gives `Err` with the line.
pub fn load_settings(dir: &Path) -> Result<Settings, ConfigError> {
    let path = ConfigFile::Settings.path_in(dir);
    match read_text(&path)? {
        None => Ok(Settings::default()),
        Some(text) => parse_settings(&text, &path).map(|loaded| loaded.value),
    }
}

/// Reads `shortcuts.toml` in `dir`. A missing file gives no shortcuts.
pub fn load_shortcuts(dir: &Path) -> Result<Vec<Shortcut>, ConfigError> {
    let path = ConfigFile::Shortcuts.path_in(dir);
    match read_text(&path)? {
        None => Ok(Vec::new()),
        Some(text) => parse_shortcuts(&text, &path).map(|loaded| loaded.value),
    }
}

/// The text of the file, or `None` when the file does not exist.
pub(crate) fn read_text(path: &Path) -> Result<Option<String>, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ConfigError::io(path, &error)),
    }
}

pub(crate) fn parse_settings(text: &str, path: &Path) -> Result<Loaded<Settings>, ConfigError> {
    let (document, value) = deserialize::<Settings>(text, path)?;
    let problems = validate_settings(&value);
    let unknown = unknown_keys(&document, ConfigFile::Settings);
    finish(
        ConfigFile::Settings,
        &document,
        path,
        value,
        problems,
        unknown,
    )
}

pub(crate) fn parse_shortcuts(
    text: &str,
    path: &Path,
) -> Result<Loaded<Vec<Shortcut>>, ConfigError> {
    let (document, file) = deserialize::<ShortcutsFile>(text, path)?;
    let problems = validate_shortcuts(&file.shortcut);
    let unknown = unknown_keys(&document, ConfigFile::Shortcuts);
    finish(
        ConfigFile::Shortcuts,
        &document,
        path,
        file.shortcut,
        problems,
        unknown,
    )
}

/// Adds the line of each problem in `text`, where the line is known.
pub(crate) fn add_lines(text: &str, problems: &mut [Problem]) {
    if let Ok(document) = Document::parse(text.to_owned()) {
        for problem in problems {
            problem.line = problem.line.or_else(|| line_of_problem(&document, problem));
        }
    }
}

pub(crate) fn deserialize<T: serde::de::DeserializeOwned>(
    text: &str,
    path: &Path,
) -> Result<(Document<String>, T), ConfigError> {
    let toml_error = |span: Option<Range<usize>>, message: &str| {
        let position = span.map(|span| line_column(text, span.start));
        ConfigError::Toml {
            path: path.to_owned(),
            line: position.map(|(line, _)| line),
            column: position.map(|(_, column)| column),
            message: message.to_owned(),
        }
    };
    let document = Document::parse(text.to_owned())
        .map_err(|error| toml_error(error.span(), error.message()))?;
    let value = toml_edit::de::from_document(document.clone())
        .map_err(|error| toml_error(error.span(), error.message()))?;
    Ok((document, value))
}

fn finish<T>(
    file: ConfigFile,
    document: &Document<String>,
    path: &Path,
    value: T,
    problems: Vec<Problem>,
    unknown_keys: Vec<Problem>,
) -> Result<Loaded<T>, ConfigError> {
    let mut problems = problems;
    for problem in &mut problems {
        problem.line = line_of_problem(document, problem);
    }
    let (errors, mut warnings): (Vec<Problem>, Vec<Problem>) =
        problems.into_iter().partition(Problem::is_error);
    if !errors.is_empty() {
        return Err(ConfigError::Invalid {
            path: path.to_owned(),
            problems: errors,
        });
    }
    warnings.extend(unknown_keys);
    debug_assert!(warnings.iter().all(|warning| warning.file == file));
    Ok(Loaded { value, warnings })
}

/// The line and the column, from 1, of the byte `offset` in `text`.
fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    let column = before[line_start..].chars().count() + 1;
    (line, column)
}

fn line_of_span(document: &Document<String>, span: Option<Range<usize>>) -> Option<usize> {
    span.map(|span| line_column(document.raw(), span.start).0)
}

fn line_of_problem(document: &Document<String>, problem: &Problem) -> Option<usize> {
    let root = document.as_table();
    let value_span = |table: &str, key: &str| {
        root.get(table)
            .and_then(Item::as_table_like)
            .and_then(|table| table.get(key))
            .and_then(Item::span)
    };
    let shortcut_span = |index: usize| {
        let table = root
            .get("shortcut")
            .and_then(Item::as_array_of_tables)
            .and_then(|tables| tables.get(index))?;
        table
            .get("keyword")
            .and_then(Item::span)
            .or_else(|| table.span())
    };
    let span = match &problem.kind {
        ProblemKind::WindowWidthOutOfRange { .. } => root.get("window_width").and_then(Item::span),
        ProblemKind::CustomPositionOutOfRange => root.get("custom_position").and_then(Item::span),
        ProblemKind::NoCustomPosition => root.get("window_position").and_then(Item::span),
        ProblemKind::EmptyKeyword { owner } | ProblemKind::KeywordWithSpace { owner, .. } => {
            match owner {
                KeywordOwner::BuiltIn(BuiltIn::FileSearch) => value_span("file_search", "keyword"),
                KeywordOwner::BuiltIn(BuiltIn::Dictionary) => value_span("dictionary", "keyword"),
                KeywordOwner::Shortcut(shortcut) => shortcut_span(shortcut.index),
            }
        }
        ProblemKind::SameBuiltInKeyword { .. } => value_span("dictionary", "keyword"),
        ProblemKind::DuplicateKeyword { shortcut, .. }
        | ProblemKind::KeywordOfBuiltIn { shortcut, .. } => shortcut_span(shortcut.index),
        ProblemKind::UnknownKey { .. } => None,
    };
    line_of_span(document, span)
}

fn unknown_keys(document: &Document<String>, file: ConfigFile) -> Vec<Problem> {
    let mut found = Vec::new();
    let root = document.as_table();
    match file {
        ConfigFile::Settings => {
            unknown_keys_in(root, SETTINGS_KEYS, "", &mut found);
            for (table, keys) in [
                ("file_search", FILE_SEARCH_KEYS),
                ("dictionary", DICTIONARY_KEYS),
                ("custom_position", CUSTOM_POSITION_KEYS),
            ] {
                if let Some(child) = root.get(table).and_then(Item::as_table_like) {
                    unknown_keys_in(child, keys, &format!("{table}."), &mut found);
                }
            }
        }
        ConfigFile::Shortcuts => {
            unknown_keys_in(root, SHORTCUTS_KEYS, "", &mut found);
            if let Some(tables) = root.get("shortcut").and_then(Item::as_array_of_tables) {
                for table in tables.iter() {
                    unknown_keys_in(table, SHORTCUT_KEYS, "shortcut.", &mut found);
                }
            }
        }
    }
    found
        .into_iter()
        .map(|(key, span)| Problem {
            file,
            line: line_of_span(document, span),
            kind: ProblemKind::UnknownKey { key },
        })
        .collect()
}

fn unknown_keys_in(
    table: &dyn TableLike,
    known: &[&str],
    prefix: &str,
    found: &mut Vec<(String, Option<Range<usize>>)>,
) {
    for (key, _) in table.iter() {
        if !known.contains(&key) {
            let span = table.key(key).and_then(|key| key.span());
            found.push((format!("{prefix}{key}"), span));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::model::{Browser, KeepQuery};
    use crate::validate::ShortcutRef;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::tempdir().expect("temporary directory")
    }

    fn write(dir: &Path, file: ConfigFile, text: &str) -> PathBuf {
        let path = file.path_in(dir);
        fs::write(&path, text).expect("write test file");
        path
    }

    #[test]
    fn gives_the_initial_settings_when_the_file_does_not_exist() {
        let dir = temp_dir();

        let settings = load_settings(dir.path());

        assert_eq!(settings, Ok(Settings::default()));
    }

    #[test]
    fn gives_no_shortcuts_when_the_file_does_not_exist() {
        let dir = temp_dir();

        assert_eq!(load_shortcuts(dir.path()), Ok(vec![]));
    }

    #[test]
    fn gives_initial_values_for_the_keys_that_the_file_does_not_have() {
        let dir = temp_dir();
        write(
            dir.path(),
            ConfigFile::Settings,
            "# my settings\nwindow_width = 900\nkeep_query = \"5m\"\n\n[file_search]\nignore = [\"~/tmp\"]\n",
        );

        let settings = load_settings(dir.path()).expect("settings");

        let mut expected = Settings {
            window_width: 900,
            keep_query: KeepQuery::Seconds(300),
            ..Settings::default()
        };
        expected.file_search.ignore = vec!["~/tmp".to_owned()];
        assert_eq!(settings, expected);
    }

    #[test]
    fn reads_each_shortcut_block() {
        let dir = temp_dir();
        write(
            dir.path(),
            ConfigFile::Shortcuts,
            r#"
[[shortcut]]
name = "Pull requests"
keyword = "pr"
cmd = "https://example.com/pull/%s"
cmd_without_argument = "https://example.com/pulls"
browser = { app = "example-browser.desktop" }

[[shortcut]]
name = "Script"
keyword = "sc"
cmd = """
#!/bin/sh
echo "$1"
"""
run_without_argument = true
"#,
        );

        let shortcuts = load_shortcuts(dir.path()).expect("shortcuts");

        assert_eq!(
            shortcuts,
            vec![
                Shortcut {
                    name: "Pull requests".to_owned(),
                    keyword: "pr".to_owned(),
                    cmd: "https://example.com/pull/%s".to_owned(),
                    cmd_without_argument: "https://example.com/pulls".to_owned(),
                    browser: Some(Browser::App("example-browser.desktop".to_owned())),
                    ..Shortcut::default()
                },
                Shortcut {
                    name: "Script".to_owned(),
                    keyword: "sc".to_owned(),
                    cmd: "#!/bin/sh\necho \"$1\"\n".to_owned(),
                    run_without_argument: true,
                    ..Shortcut::default()
                },
            ]
        );
    }

    #[test]
    fn gives_the_line_and_column_of_a_toml_syntax_error() {
        let dir = temp_dir();
        let path = write(
            dir.path(),
            ConfigFile::Settings,
            "# comment\nwindow_width = 800\nhotkey = <Super>space\n",
        );

        let error = load_settings(dir.path()).expect_err("syntax error");

        let ConfigError::Toml {
            path: error_path,
            line,
            column,
            message,
        } = error
        else {
            panic!("not a TOML error: {error:?}");
        };
        assert_eq!(error_path, path);
        assert_eq!((line, column), (Some(3), Some(10)));
        assert!(!message.is_empty());
    }

    #[test]
    fn gives_the_line_of_a_value_with_the_wrong_type() {
        let cases = [
            "window_width = \"wide\"\n",
            "close_on_focus_loss = \"yes\"\n",
            "keep_query = \"soon\"\n",
            "[file_search]\nignore = \"~/tmp\"\n",
        ];
        for text in cases {
            let dir = temp_dir();
            let lines = text.lines().count();
            write(dir.path(), ConfigFile::Settings, text);

            let error = load_settings(dir.path()).expect_err(text);

            assert!(
                matches!(error, ConfigError::Toml { .. }),
                "{text}: {error:?}"
            );
            assert_eq!(error.line(), Some(lines), "{text}");
        }
    }

    #[test]
    fn gives_the_line_of_a_wrong_shortcut_value() {
        let dir = temp_dir();
        write(
            dir.path(),
            ConfigFile::Shortcuts,
            "[[shortcut]]\nname = \"A\"\nkeyword = \"a\"\n\n[[shortcut]]\nname = \"B\"\nbrowser = { firefox = \"x\" }\n",
        );

        let error = load_shortcuts(dir.path()).expect_err("unknown browser kind");

        assert_eq!(error.line(), Some(7));
        assert!(error.to_string().contains("firefox"), "{error}");
    }

    #[test]
    fn gives_the_line_of_a_validation_error() {
        let dir = temp_dir();
        let path = write(
            dir.path(),
            ConfigFile::Settings,
            "hotkey = \"<Super>space\"\nwindow_width = 75\n",
        );

        let error = load_settings(dir.path()).expect_err("too narrow");

        assert_eq!(
            error,
            ConfigError::Invalid {
                path,
                problems: vec![Problem {
                    file: ConfigFile::Settings,
                    line: Some(2),
                    kind: ProblemKind::WindowWidthOutOfRange { width: 75 },
                }],
            }
        );
    }

    #[test]
    fn gives_the_keyword_line_of_a_shortcut_error() {
        let dir = temp_dir();
        write(
            dir.path(),
            ConfigFile::Shortcuts,
            "[[shortcut]]\nname = \"A\"\nkeyword = \"a\"\n\n[[shortcut]]\nname = \"B\"\nkeyword = \"b c\"\n\n[[shortcut]]\nname = \"C\"\n",
        );

        let error = load_shortcuts(dir.path()).expect_err("keyword errors");

        let ConfigError::Invalid { problems, .. } = error else {
            panic!("not invalid: {error:?}");
        };
        let lines: Vec<Option<usize>> = problems.iter().map(|problem| problem.line).collect();
        assert_eq!(lines, vec![Some(7), Some(9)]);
    }

    #[test]
    fn warns_about_unknown_keys_with_their_lines() {
        let path = PathBuf::from("settings.toml");
        let text = "windw_width = 800\n\n[file_search]\nkeyword = \"f\"\nignores = []\n\n[colors]\nx = 1\n";

        let loaded = parse_settings(text, &path).expect("valid file");

        let unknown: Vec<(String, Option<usize>)> = loaded
            .warnings
            .iter()
            .map(|warning| (warning.kind.to_string(), warning.line))
            .collect();
        assert_eq!(
            unknown,
            vec![
                (
                    "unknown key \"windw_width\" has no effect".to_owned(),
                    Some(1)
                ),
                ("unknown key \"colors\" has no effect".to_owned(), Some(7)),
                (
                    "unknown key \"file_search.ignores\" has no effect".to_owned(),
                    Some(5)
                ),
            ]
        );
    }

    #[test]
    fn warns_about_unknown_shortcut_keys_and_duplicates() {
        let text = "[[shortcut]]\nname = \"A\"\nkeyword = \"a\"\ncomd = \"x\"\n\n[[shortcut]]\nname = \"B\"\nkeyword = \"a\"\n";

        let loaded = parse_shortcuts(text, Path::new("shortcuts.toml")).expect("valid file");

        assert_eq!(
            loaded.warnings,
            vec![
                Problem {
                    file: ConfigFile::Shortcuts,
                    line: Some(8),
                    kind: ProblemKind::DuplicateKeyword {
                        keyword: "a".to_owned(),
                        first: ShortcutRef {
                            index: 0,
                            name: "A".to_owned()
                        },
                        shortcut: ShortcutRef {
                            index: 1,
                            name: "B".to_owned()
                        },
                    },
                },
                Problem {
                    file: ConfigFile::Shortcuts,
                    line: Some(4),
                    kind: ProblemKind::UnknownKey {
                        key: "shortcut.comd".to_owned()
                    },
                },
            ]
        );
    }

    #[test]
    fn knows_each_key_of_the_model() {
        let settings = toml_edit::ser::to_string(&Settings::default()).expect("serialize");
        let full = Shortcut {
            browser: Some(Browser::Command("x".to_owned())),
            ..Shortcut::default()
        };
        let shortcuts = toml_edit::ser::to_string(&ShortcutsFile {
            shortcut: vec![full],
        })
        .expect("serialize");

        let settings = parse_settings(&settings, Path::new("s")).expect("settings");
        let shortcuts = parse_shortcuts(&shortcuts, Path::new("s"));

        assert_eq!(settings.warnings, vec![]);
        let unknown: Vec<Problem> = match shortcuts {
            Ok(loaded) => loaded.warnings,
            Err(ConfigError::Invalid { .. }) => Vec::new(),
            Err(error) => panic!("{error}"),
        };
        assert!(
            unknown
                .iter()
                .all(|problem| !matches!(problem.kind, ProblemKind::UnknownKey { .. })),
            "{unknown:?}"
        );
    }

    #[test]
    fn adds_the_line_of_a_shortcut_to_a_problem_from_another_check() {
        let text = "[[shortcut]]\nname = \"A\"\nkeyword = \"a\"\n\n[[shortcut]]\nname = \"D\"\n\nkeyword = \"d\"\n";
        let mut problems = vec![Problem {
            file: ConfigFile::Shortcuts,
            line: None,
            kind: ProblemKind::KeywordOfBuiltIn {
                keyword: "d".to_owned(),
                shortcut: ShortcutRef {
                    index: 1,
                    name: "D".to_owned(),
                },
                built_in: BuiltIn::Dictionary,
            },
        }];

        add_lines(text, &mut problems);

        assert_eq!(problems[0].line, Some(8));
    }

    #[test]
    fn gives_an_io_error_when_the_file_cannot_be_read() {
        let dir = temp_dir();
        fs::create_dir(ConfigFile::Settings.path_in(dir.path())).expect("directory");

        let error = load_settings(dir.path()).expect_err("a directory is not a file");

        assert!(matches!(error, ConfigError::Io { .. }), "{error:?}");
    }

    #[test]
    fn counts_the_column_in_characters() {
        assert_eq!(line_column("ä = 1\nöö = x", 12), (2, 4));
        assert_eq!(line_column("a", 99), (1, 2));
    }
}
