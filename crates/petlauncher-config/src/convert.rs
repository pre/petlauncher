//! The one-time conversion of the Ulauncher shortcuts to `shortcuts.toml`
//! (R6), for the command `petlauncher convert-shortcuts`.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::Deserialize;

use crate::edit::ShortcutsEditor;
use crate::error::ConfigError;
use crate::load::{add_lines, load_settings};
use crate::model::{Settings, Shortcut};
use crate::paths::{config_dir, ulauncher_shortcuts_file, ConfigFile};
use crate::validate::{validate_across, validate_shortcuts, BuiltIn, Problem};
use crate::write::write_atomic;

/// The shortcuts from the Ulauncher file, in the order of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    pub shortcuts: Vec<Shortcut>,
    /// The shortcuts that got a new keyword.
    pub renamed: Vec<RenamedKeyword>,
}

/// A shortcut that had the keyword of a built-in search, for example `d`
/// of the dictionary (R19), and got the keyword two times, `dd`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenamedKeyword {
    pub name: String,
    pub from: String,
    pub to: String,
    pub built_in: BuiltIn,
}

/// The result of [`convert_shortcuts_file`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertSummary {
    /// The `shortcuts.toml` that was written.
    pub path: PathBuf,
    pub count: usize,
    pub renamed: Vec<RenamedKeyword>,
    /// The errors and warnings of the new file, from
    /// [`crate::validate::validate`]. An error stops the new file at the
    /// next start of petlauncher, and the owner must correct it.
    pub problems: Vec<Problem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertError {
    /// The settings directory or the Ulauncher file is not known, because
    /// `HOME` is not set.
    NoHome,
    /// The Ulauncher file cannot be read.
    Read { path: PathBuf, message: String },
    /// The Ulauncher file is not a JSON object of shortcuts.
    Json {
        line: usize,
        column: usize,
        message: String,
    },
    /// `shortcuts.toml` exists, and the force flag is off.
    TargetExists(PathBuf),
    /// `settings.toml` has an error, or `shortcuts.toml` cannot be written.
    Config(ConfigError),
}

/// Converts the text of the Ulauncher file `shortcuts.json`. `settings`
/// gives the keywords of the built-in searches.
///
/// Each field with the same name keeps its value. `id` and `added` are
/// not used. A keyword that is the keyword of the file search or of the
/// dictionary is written two times, or more times when that keyword is
/// also used. A script with more than one line gets a line end at its end,
/// so that `shortcuts.toml` shows it as a readable multi-line string. A
/// script that opens a URL in a different browser stays a script.
pub fn convert_ulauncher_shortcuts(
    json: &str,
    settings: &Settings,
) -> Result<Conversion, ConvertError> {
    let InOrder(items) = serde_json::from_str(json).map_err(|error| {
        let position = format!(" at line {} column {}", error.line(), error.column());
        let message = error.to_string();
        ConvertError::Json {
            line: error.line(),
            column: error.column(),
            message: message
                .strip_suffix(&position)
                .unwrap_or(&message)
                .to_owned(),
        }
    })?;
    let built_ins = [
        (BuiltIn::FileSearch, settings.file_search.keyword.as_str()),
        (BuiltIn::Dictionary, settings.dictionary.keyword.as_str()),
    ];
    let original: Vec<String> = items.iter().map(|item| text(&item.keyword)).collect();
    let mut renamed = Vec::new();
    let mut shortcuts: Vec<Shortcut> = Vec::new();
    for item in items {
        let mut shortcut = item.into_shortcut();
        let conflict = built_ins
            .iter()
            .find(|(_, keyword)| !keyword.is_empty() && *keyword == shortcut.keyword);
        if let Some((built_in, _)) = conflict {
            let from = shortcut.keyword.clone();
            let mut to = format!("{from}{from}");
            let is_used = |keyword: &str| {
                built_ins.iter().any(|(_, used)| *used == keyword)
                    || original.iter().any(|used| used == keyword)
                    || shortcuts.iter().any(|done| done.keyword == keyword)
            };
            while is_used(&to) {
                to.push_str(&from);
            }
            renamed.push(RenamedKeyword {
                name: shortcut.name.clone(),
                from,
                to: to.clone(),
                built_in: *built_in,
            });
            shortcut.keyword = to;
        }
        shortcuts.push(shortcut);
    }
    Ok(Conversion { shortcuts, renamed })
}

/// Reads the Ulauncher file and writes `shortcuts.toml` in `config_dir`.
/// Without `force`, an existing `shortcuts.toml` gives
/// [`ConvertError::TargetExists`] and nothing is written. The Ulauncher
/// file does not change.
pub fn convert_shortcuts_file(
    ulauncher_file: &Path,
    config_dir: &Path,
    force: bool,
) -> Result<ConvertSummary, ConvertError> {
    let target = ConfigFile::Shortcuts.path_in(config_dir);
    if !force && fs::symlink_metadata(&target).is_ok() {
        return Err(ConvertError::TargetExists(target));
    }
    let json = fs::read_to_string(ulauncher_file).map_err(|error| ConvertError::Read {
        path: ulauncher_file.to_owned(),
        message: error.to_string(),
    })?;
    let settings = load_settings(config_dir).map_err(ConvertError::Config)?;
    let conversion = convert_ulauncher_shortcuts(&json, &settings)?;
    let mut editor = ShortcutsEditor::new_file();
    for shortcut in &conversion.shortcuts {
        editor.add(shortcut);
    }
    let text = editor.text();
    write_atomic(&target, &text).map_err(ConvertError::Config)?;
    let mut problems = validate_shortcuts(&conversion.shortcuts);
    problems.extend(validate_across(&settings, &conversion.shortcuts));
    add_lines(&text, &mut problems);
    Ok(ConvertSummary {
        path: target,
        count: conversion.shortcuts.len(),
        renamed: conversion.renamed,
        problems,
    })
}

/// The command `petlauncher convert-shortcuts`: converts
/// `~/.config/ulauncher/shortcuts.json` to
/// `~/.config/petlauncher/shortcuts.toml`.
pub fn convert_shortcuts(force: bool) -> Result<ConvertSummary, ConvertError> {
    let (Some(ulauncher_file), Some(config_dir)) = (ulauncher_shortcuts_file(), config_dir())
    else {
        return Err(ConvertError::NoHome);
    };
    convert_shortcuts_file(&ulauncher_file, &config_dir, force)
}

/// One value of the Ulauncher file. The string fields accept `null`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct UlauncherShortcut {
    name: Option<String>,
    keyword: Option<String>,
    cmd: Option<String>,
    icon: Option<String>,
    is_default_search: bool,
    run_without_argument: bool,
}

impl UlauncherShortcut {
    fn into_shortcut(self) -> Shortcut {
        let mut cmd = text(&self.cmd);
        if cmd.contains('\n') && !cmd.ends_with('\n') {
            cmd.push('\n');
        }
        Shortcut {
            name: text(&self.name),
            keyword: text(&self.keyword),
            cmd,
            icon: text(&self.icon),
            is_default_search: self.is_default_search,
            run_without_argument: self.run_without_argument,
            ..Shortcut::default()
        }
    }
}

fn text(value: &Option<String>) -> String {
    value.clone().unwrap_or_default()
}

/// The values of the JSON object in the order of the file.
struct InOrder(Vec<UlauncherShortcut>);

impl<'de> Deserialize<'de> for InOrder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;

        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = InOrder;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object with one shortcut for each ID")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<InOrder, A::Error> {
                let mut items = Vec::new();
                while let Some((_id, item)) = map.next_entry::<String, UlauncherShortcut>()? {
                    items.push(item);
                }
                Ok(InOrder(items))
            }
        }

        deserializer.deserialize_map(ObjectVisitor)
    }
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHome => f.write_str("the home directory is not known, because HOME is not set"),
            Self::Read { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Json {
                line,
                column,
                message,
            } => write!(f, "line {line}, column {column}: {message}"),
            Self::TargetExists(path) => write!(
                f,
                "{} exists. Use the force flag to write over it",
                path.display()
            ),
            Self::Config(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ConvertError {}

impl fmt::Display for ConvertSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "Wrote {} shortcuts to {}.",
            self.count,
            self.path.display()
        )?;
        for renamed in &self.renamed {
            writeln!(
                f,
                "The shortcut \"{}\" has the keyword \"{}\", because \"{}\" is the keyword of {}.",
                renamed.name, renamed.to, renamed.from, renamed.built_in
            )?;
        }
        for problem in &self.problems {
            writeln!(f, "{problem}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(keyword: &str, cmd: &str) -> String {
        format!(
            r#""id-{keyword}": {{"added": 0, "cmd": {cmd:?}, "icon": "", "id": "id-{keyword}",
            "is_default_search": false, "keyword": "{keyword}", "name": "Name {keyword}",
            "run_without_argument": false}}"#
        )
    }

    fn json(entries: &[String]) -> String {
        format!("{{{}}}", entries.join(",\n"))
    }

    fn keywords(conversion: &Conversion) -> Vec<&str> {
        conversion
            .shortcuts
            .iter()
            .map(|shortcut| shortcut.keyword.as_str())
            .collect()
    }

    #[test]
    fn converts_each_field_in_the_order_of_the_file() {
        let text = r#"{
            "b": {"added": 1700000000.5, "cmd": "https://x.example.com/%s", "icon": "x.png",
                  "id": "b", "is_default_search": true, "keyword": "x", "name": "X",
                  "run_without_argument": false},
            "a": {"added": 0, "cmd": "https://y.example.com/", "icon": "", "id": "a",
                  "is_default_search": false, "keyword": "y", "name": "Y",
                  "run_without_argument": true}
        }"#;

        let conversion = convert_ulauncher_shortcuts(text, &Settings::default()).expect("valid");

        assert_eq!(
            conversion.shortcuts,
            vec![
                Shortcut {
                    name: "X".to_owned(),
                    keyword: "x".to_owned(),
                    cmd: "https://x.example.com/%s".to_owned(),
                    icon: "x.png".to_owned(),
                    is_default_search: true,
                    ..Shortcut::default()
                },
                Shortcut {
                    name: "Y".to_owned(),
                    keyword: "y".to_owned(),
                    cmd: "https://y.example.com/".to_owned(),
                    run_without_argument: true,
                    ..Shortcut::default()
                },
            ]
        );
        assert_eq!(conversion.renamed, vec![]);
    }

    #[test]
    fn gives_the_shortcut_with_the_dictionary_keyword_the_keyword_dd() {
        let text = json(&[entry("d", "https://dict.example.com/%s")]);

        let conversion = convert_ulauncher_shortcuts(&text, &Settings::default()).expect("valid");

        assert_eq!(keywords(&conversion), ["dd"]);
        assert_eq!(
            conversion.renamed,
            vec![RenamedKeyword {
                name: "Name d".to_owned(),
                from: "d".to_owned(),
                to: "dd".to_owned(),
                built_in: BuiltIn::Dictionary,
            }]
        );
    }

    #[test]
    fn uses_the_keywords_of_the_settings_and_a_keyword_that_is_not_used() {
        let mut settings = Settings::default();
        settings.file_search.keyword = "x".to_owned();
        let text = json(&[entry("x", "a"), entry("xx", "b"), entry("d", "c")]);

        let conversion = convert_ulauncher_shortcuts(&text, &settings).expect("valid");

        assert_eq!(keywords(&conversion), ["xxx", "xx", "dd"]);
        assert_eq!(conversion.renamed[0].built_in, BuiltIn::FileSearch);
    }

    #[test]
    fn ends_a_script_of_more_lines_with_a_line_end_and_keeps_it_a_script() {
        let script = "#!/bin/sh\nexec example-browser \"https://review.example.com/$1\"";
        let text = json(&[entry("rv", script)]);

        let conversion = convert_ulauncher_shortcuts(&text, &Settings::default()).expect("valid");

        assert_eq!(conversion.shortcuts[0].cmd, format!("{script}\n"));
        assert_eq!(conversion.shortcuts[0].browser, None);
        assert_eq!(conversion.shortcuts[0].cmd_without_argument, "");
    }

    #[test]
    fn accepts_missing_and_null_fields() {
        let text = r#"{"a": {"keyword": "a", "cmd": "https://a.example.com/", "icon": null}}"#;

        let conversion = convert_ulauncher_shortcuts(text, &Settings::default()).expect("valid");

        assert_eq!(conversion.shortcuts[0].icon, "");
        assert_eq!(conversion.shortcuts[0].name, "");
    }

    #[test]
    fn gives_the_line_of_a_json_error() {
        let text = "{\n  \"a\": {\"keyword\": 5}\n}";

        let error =
            convert_ulauncher_shortcuts(text, &Settings::default()).expect_err("wrong type");

        let ConvertError::Json { line, .. } = error else {
            panic!("not a JSON error: {error:?}");
        };
        assert_eq!(line, 2);
    }

    #[test]
    fn rejects_a_file_that_is_not_an_object() {
        let error = convert_ulauncher_shortcuts("[]", &Settings::default()).expect_err("a list");

        assert!(error.to_string().contains("object"), "{error}");
    }

    #[test]
    fn writes_a_summary_that_names_the_new_keywords() {
        let summary = ConvertSummary {
            path: PathBuf::from("/c/shortcuts.toml"),
            count: 9,
            renamed: vec![RenamedKeyword {
                name: "Cambridge".to_owned(),
                from: "d".to_owned(),
                to: "dd".to_owned(),
                built_in: BuiltIn::Dictionary,
            }],
            problems: vec![],
        };

        assert_eq!(
            summary.to_string(),
            "Wrote 9 shortcuts to /c/shortcuts.toml.\n\
             The shortcut \"Cambridge\" has the keyword \"dd\", because \"d\" is the keyword of the dictionary.\n"
        );
    }

    #[test]
    fn writes_each_error() {
        let cases = [
            (
                ConvertError::NoHome,
                "the home directory is not known, because HOME is not set",
            ),
            (
                ConvertError::Read {
                    path: PathBuf::from("/u/shortcuts.json"),
                    message: "not found".to_owned(),
                },
                "/u/shortcuts.json: not found",
            ),
            (
                ConvertError::Json {
                    line: 2,
                    column: 7,
                    message: "bad".to_owned(),
                },
                "line 2, column 7: bad",
            ),
            (
                ConvertError::TargetExists(PathBuf::from("/c/shortcuts.toml")),
                "/c/shortcuts.toml exists. Use the force flag to write over it",
            ),
        ];

        for (error, message) in cases {
            assert_eq!(error.to_string(), message);
        }
    }
}
