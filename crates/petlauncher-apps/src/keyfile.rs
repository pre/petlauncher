//! Reader for the key file format of desktop entries. Only the group
//! `[Desktop Entry]` is read.

use std::collections::HashMap;

use crate::locale::Locale;

/// The keys of the group `[Desktop Entry]`. A localized key keeps its
/// suffix, for example `Name[fi]`. The values keep their escape sequences.
#[derive(Debug, Default)]
pub(crate) struct Group {
    values: HashMap<String, String>,
}

/// Gives `None` when the text has no group `[Desktop Entry]`.
pub(crate) fn desktop_entry_group(text: &str) -> Option<Group> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut group: Option<Group> = None;
    let mut in_group = false;
    for line in text.lines() {
        let line = line.trim_start();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line
            .trim_end()
            .strip_prefix('[')
            .and_then(|l| l.strip_suffix(']'))
        {
            in_group = name == "Desktop Entry";
            if in_group && group.is_none() {
                group = Some(Group::default());
            }
            continue;
        }
        if !in_group {
            continue;
        }
        if let (Some(group), Some((key, value))) = (group.as_mut(), line.split_once('=')) {
            group
                .values
                .insert(key.trim_end().to_string(), value.trim().to_string());
        }
    }
    group
}

impl Group {
    pub(crate) fn string(&self, key: &str) -> Option<String> {
        self.values.get(key).map(|value| unescape(value))
    }

    pub(crate) fn localized(&self, key: &str, locale: Option<&Locale>) -> Option<String> {
        self.localized_raw(key, locale).map(unescape)
    }

    pub(crate) fn list(&self, key: &str) -> Vec<String> {
        self.values
            .get(key)
            .map_or_else(Vec::new, |value| split_list(value))
    }

    pub(crate) fn localized_list(&self, key: &str, locale: Option<&Locale>) -> Vec<String> {
        self.localized_raw(key, locale)
            .map_or_else(Vec::new, split_list)
    }

    /// `true` for the values `true` and `1`, as GLib reads them. A missing
    /// key is `false`.
    pub(crate) fn boolean(&self, key: &str) -> bool {
        matches!(self.values.get(key).map(String::as_str), Some("true" | "1"))
    }

    fn localized_raw(&self, key: &str, locale: Option<&Locale>) -> Option<&str> {
        locale
            .into_iter()
            .flat_map(Locale::suffixes)
            .find_map(|suffix| self.values.get(&format!("{key}[{suffix}]")))
            .or_else(|| self.values.get(key))
            .map(String::as_str)
    }
}

/// Undoes `\s`, `\n`, `\t`, `\r` and `\\`. Keeps other escape sequences, so
/// the `Exec` quoting can read `\"` and `\$`.
fn unescape(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => result.push(' '),
            Some('n') => result.push('\n'),
            Some('t') => result.push('\t'),
            Some('r') => result.push('\r'),
            Some('\\') => result.push('\\'),
            Some(other) => {
                result.push('\\');
                result.push(other);
            }
            None => result.push('\\'),
        }
    }
    result
}

/// Splits at `;` that has no backslash before it. Empty items are removed.
fn split_list(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut item = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(';') => item.push(';'),
                Some(other) => {
                    item.push('\\');
                    item.push(other);
                }
                None => item.push('\\'),
            },
            ';' => items.push(std::mem::take(&mut item)),
            _ => item.push(c),
        }
    }
    items.push(item);
    items
        .iter()
        .map(|item| unescape(item))
        .filter(|item| !item.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(text: &str) -> Group {
        desktop_entry_group(text).expect("group [Desktop Entry]")
    }

    #[test]
    fn reads_keys_of_the_desktop_entry_group_only() {
        let text = "# comment\n\n[Desktop Entry]\nName=Files\nExec=nautilus\n\n\
                    [Desktop Action new-window]\nName=New Window\n";

        let group = group(text);

        assert_eq!(group.string("Name").as_deref(), Some("Files"));
        assert_eq!(group.string("Exec").as_deref(), Some("nautilus"));
    }

    #[test]
    fn gives_none_without_a_desktop_entry_group() {
        assert!(desktop_entry_group("[Other]\nName=x\n").is_none());
        assert!(desktop_entry_group("").is_none());
    }

    #[test]
    fn ignores_spaces_around_the_equals_sign_and_comment_lines() {
        let group = group("[Desktop Entry]\n  # a comment\nName = Text Editor\nIcon= x \n");

        assert_eq!(group.string("Name").as_deref(), Some("Text Editor"));
        assert_eq!(group.string("Icon").as_deref(), Some("x"));
    }

    #[test]
    fn reads_a_file_with_windows_line_ends_and_a_byte_order_mark() {
        let group = group("\u{feff}[Desktop Entry]\r\nName=Files\r\n");

        assert_eq!(group.string("Name").as_deref(), Some("Files"));
    }

    #[test]
    fn undoes_the_escape_sequences_of_a_string() {
        let group = group("[Desktop Entry]\nComment=a\\sb\\tc\\nd\\re\\\\f\n");

        assert_eq!(group.string("Comment").as_deref(), Some("a b\tc\nd\re\\f"));
    }

    #[test]
    fn keeps_an_unknown_escape_sequence() {
        let group = group("[Desktop Entry]\nExec=sh -c \"echo \\$HOME\"\n");

        assert_eq!(
            group.string("Exec").as_deref(),
            Some("sh -c \"echo \\$HOME\"")
        );
    }

    #[test]
    fn splits_a_list_at_semicolons_that_are_not_escaped() {
        let group = group("[Desktop Entry]\nKeywords=shell;prompt\\;line;;cmd;\n");

        assert_eq!(group.list("Keywords"), ["shell", "prompt;line", "cmd"]);
    }

    #[test]
    fn gives_an_empty_list_for_a_missing_key() {
        assert!(group("[Desktop Entry]\n").list("Categories").is_empty());
    }

    #[test]
    fn selects_the_best_localized_value() {
        let text = "[Desktop Entry]\nName=Files\nName[fi]=Tiedostot\nName[fi_FI]=Tiedostot FI\n\
                    Name[sv]=Filer\n";
        let group = group(text);
        let finnish_finland = Locale::parse("fi_FI.UTF-8");
        let finnish_aland = Locale::parse("fi_AX.UTF-8");
        let german = Locale::parse("de_DE.UTF-8");

        assert_eq!(
            group.localized("Name", finnish_finland.as_ref()).as_deref(),
            Some("Tiedostot FI")
        );
        assert_eq!(
            group.localized("Name", finnish_aland.as_ref()).as_deref(),
            Some("Tiedostot")
        );
        assert_eq!(
            group.localized("Name", german.as_ref()).as_deref(),
            Some("Files")
        );
        assert_eq!(group.localized("Name", None).as_deref(), Some("Files"));
    }

    #[test]
    fn selects_the_localized_list() {
        let group = group("[Desktop Entry]\nKeywords=folder;\nKeywords[fi]=kansio;hakemisto;\n");

        assert_eq!(
            group.localized_list("Keywords", Locale::parse("fi_FI").as_ref()),
            ["kansio", "hakemisto"]
        );
        assert_eq!(group.localized_list("Keywords", None), ["folder"]);
    }

    #[test]
    fn reads_true_and_1_as_true() {
        let group =
            group("[Desktop Entry]\nNoDisplay=true\nHidden=false\nTerminal=1\nStartupNotify=yes\n");

        assert!(group.boolean("NoDisplay"));
        assert!(!group.boolean("Hidden"));
        assert!(group.boolean("Terminal"));
        assert!(!group.boolean("StartupNotify"));
        assert!(!group.boolean("Missing"));
    }
}
