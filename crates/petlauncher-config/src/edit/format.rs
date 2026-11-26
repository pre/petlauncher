//! Changes of TOML items that keep the spaces and comments around them.

use toml_edit::{Array, Item, TableLike, Value};

/// Sets `key` in `table`. An existing value keeps the text before and after
/// it, for example a comment at the end of the line.
pub(super) fn set_value(table: &mut dyn TableLike, key: &str, value: impl Into<Value>) {
    let mut value = value.into();
    match table.get_mut(key) {
        Some(Item::Value(old)) => {
            *value.decor_mut() = old.decor().clone();
            *old = value;
        }
        _ => {
            table.insert(key, Item::Value(value));
        }
    }
}

/// Adds `entry` at the end of `array` in the style of the last entry: on a
/// new line with the same indent, or on the same line.
///
/// In a multi-line array, the text after the last entry starts with the
/// end of its line, which can have a comment. That line end moves to the
/// front of the new entry.
pub(super) fn push_entry(array: &mut Array, entry: &str) {
    let last_prefix = array.iter().last().map(|last| prefix_of(last).to_owned());
    let prefix = match last_prefix {
        None => String::new(),
        Some(last_prefix) => match last_prefix.rfind('\n') {
            None => " ".to_owned(),
            Some(line_end) => {
                let indent = &last_prefix[line_end + 1..];
                let trailing = array.trailing().as_str().unwrap_or_default().to_owned();
                if trailing.contains('\n') {
                    let (end_of_last_line, after) = split_first_line(&trailing);
                    array.set_trailing(format!("\n{after}"));
                    format!("{end_of_last_line}{indent}")
                } else {
                    format!("\n{indent}")
                }
            }
        },
    };
    array.push_formatted(Value::from(entry).decorated(prefix, ""));
}

/// Removes the entry at `index` and the comment lines above it. A comment
/// at the end of the line of the entry before it stays.
///
/// In a multi-line array, the prefix of an entry starts with the end of
/// the line before it, and the comment lines above the entry follow.
pub(super) fn remove_entry(array: &mut Array, index: usize) {
    let removed = prefix_of(&array.remove(index)).to_owned();
    let (end_of_line_before, _) = split_first_line(&removed);
    if let Some(next) = array.get_mut(index) {
        let next_prefix = prefix_of(next).to_owned();
        let prefix = if next_prefix.contains('\n') {
            format!("{end_of_line_before}{}", split_first_line(&next_prefix).1)
        } else {
            removed.clone()
        };
        next.decor_mut().set_prefix(prefix);
    } else {
        let trailing = array.trailing().as_str().unwrap_or_default().to_owned();
        if trailing.contains('\n') {
            array.set_trailing(format!(
                "{end_of_line_before}{}",
                split_first_line(&trailing).1
            ));
        }
    }
    if array.is_empty() {
        array.set_trailing_comma(false);
        if !has_comment(array.trailing().as_str().unwrap_or_default()) {
            array.set_trailing("");
        }
    }
}

/// An array with one entry on each line, as in the template.
pub(super) fn multi_line_array(entries: &[String]) -> Array {
    let mut array = Array::new();
    if entries.is_empty() {
        return array;
    }
    for entry in entries {
        array.push_formatted(Value::from(entry.as_str()).decorated("\n    ", ""));
    }
    array.set_trailing_comma(true);
    array.set_trailing("\n");
    array
}

/// Divides the text before a table header into the part that belongs to
/// the place in the file and the part that belongs to the table. The table
/// has the comment lines directly above its header. The place has all
/// lines up to and including the last empty line.
pub(super) fn split_prefix(prefix: &str) -> (&str, &str) {
    let mut end = 0;
    let mut position = 0;
    for line in prefix.split_inclusive('\n') {
        position += line.len();
        if line.ends_with('\n') && line.trim().is_empty() {
            end = position;
        }
    }
    prefix.split_at(end)
}

pub(super) fn has_comment(text: &str) -> bool {
    text.contains('#')
}

fn prefix_of(value: &Value) -> &str {
    value
        .decor()
        .prefix()
        .and_then(|prefix| prefix.as_str())
        .unwrap_or_default()
}

/// The text up to and including the first line end, and the rest. With no
/// line end, the first part is empty.
fn split_first_line(text: &str) -> (&str, &str) {
    match text.find('\n') {
        Some(line_end) => text.split_at(line_end + 1),
        None => ("", text),
    }
}

#[cfg(test)]
mod tests {
    use toml_edit::DocumentMut;

    use super::*;

    fn parse(text: &str) -> DocumentMut {
        text.parse().expect("valid TOML")
    }

    fn array<'d>(document: &'d mut DocumentMut, key: &str) -> &'d mut Array {
        document[key].as_array_mut().expect("array")
    }

    #[test]
    fn keeps_the_comment_after_a_changed_value() {
        let mut document = parse("window_width = 750   # pixels\nx = 1\n");

        set_value(document.as_table_mut(), "window_width", 900);

        assert_eq!(
            document.to_string(),
            "window_width = 900   # pixels\nx = 1\n"
        );
    }

    #[test]
    fn adds_a_missing_key_after_the_other_keys() {
        let mut document = parse("x = 1\n\n[t]\ny = 2\n");

        set_value(document.as_table_mut(), "z", "new");

        assert_eq!(document.to_string(), "x = 1\nz = \"new\"\n\n[t]\ny = 2\n");
    }

    #[test]
    fn adds_an_entry_on_a_new_line_with_the_indent_of_the_list() {
        let mut document = parse("ignore = [\n    \"a\", # comment a\n    \"b\",\n]\n");

        push_entry(array(&mut document, "ignore"), "c");

        assert_eq!(
            document.to_string(),
            "ignore = [\n    \"a\", # comment a\n    \"b\",\n    \"c\",\n]\n"
        );
    }

    #[test]
    fn adds_an_entry_after_the_comment_on_the_line_of_the_last_entry() {
        let mut document =
            parse("ignore = [\n    \"a\",\n    \"b\", # comment b\n    # \"old\",\n]\n");

        push_entry(array(&mut document, "ignore"), "c");

        assert_eq!(
            document.to_string(),
            "ignore = [\n    \"a\",\n    \"b\", # comment b\n    \"c\",\n    # \"old\",\n]\n"
        );
    }

    #[test]
    fn adds_an_entry_to_a_multi_line_list_that_ends_on_the_line_of_the_last_entry() {
        let mut document = parse("ignore = [\n  \"a\",\n  \"b\"]\n");

        push_entry(array(&mut document, "ignore"), "c");

        assert_eq!(
            document.to_string(),
            "ignore = [\n  \"a\",\n  \"b\",\n  \"c\"]\n"
        );
    }

    #[test]
    fn adds_an_entry_on_the_same_line_of_a_one_line_list() {
        let mut document = parse("a = [\"x\"]\nb = []\n");

        push_entry(array(&mut document, "a"), "y");
        push_entry(array(&mut document, "b"), "z");

        assert_eq!(document.to_string(), "a = [\"x\", \"y\"]\nb = [\"z\"]\n");
    }

    #[test]
    fn removes_an_entry_and_keeps_the_comment_of_the_entry_before_it() {
        let text = "ignore = [\n    \"a\", # comment a\n    \"b\", # comment b\n    # about c\n    \"c\",\n]\n";
        let mut document = parse(text);

        remove_entry(array(&mut document, "ignore"), 1);

        assert_eq!(
            document.to_string(),
            "ignore = [\n    \"a\", # comment a\n    # about c\n    \"c\",\n]\n"
        );
    }

    #[test]
    fn removes_the_comment_lines_above_a_removed_entry() {
        let mut document = parse("ignore = [\n    \"a\",\n    # about b\n    \"b\",\n]\n");

        remove_entry(array(&mut document, "ignore"), 1);

        assert_eq!(document.to_string(), "ignore = [\n    \"a\",\n]\n");
    }

    #[test]
    fn removes_the_first_entry_of_a_multi_line_list() {
        let mut document = parse("ignore = [\n    \"a\", # comment a\n    \"b\",\n]\n");

        remove_entry(array(&mut document, "ignore"), 0);

        assert_eq!(document.to_string(), "ignore = [\n    \"b\",\n]\n");
    }

    #[test]
    fn removes_the_last_entry_with_its_comment() {
        let mut document = parse("ignore = [\n    \"a\",\n    \"b\", # comment b\n]\n");

        remove_entry(array(&mut document, "ignore"), 1);

        assert_eq!(document.to_string(), "ignore = [\n    \"a\",\n]\n");
    }

    #[test]
    fn removes_entries_of_a_one_line_list() {
        let mut document = parse("a = [\"x\", \"y\", \"z\"]\n");

        remove_entry(array(&mut document, "a"), 0);
        assert_eq!(document.to_string(), "a = [\"y\", \"z\"]\n");
        remove_entry(array(&mut document, "a"), 1);
        assert_eq!(document.to_string(), "a = [\"y\"]\n");
    }

    #[test]
    fn removes_the_only_entry() {
        let mut document = parse("a = [\n    \"x\",\n]\n");

        remove_entry(array(&mut document, "a"), 0);

        assert_eq!(document.to_string(), "a = []\n");
    }

    #[test]
    fn removes_the_only_entry_and_keeps_a_comment_line_of_the_list() {
        let mut document = parse("a = [\n    \"x\", # about x\n    # \"old\", keep me\n]\n");

        remove_entry(array(&mut document, "a"), 0);

        assert_eq!(document.to_string(), "a = [\n    # \"old\", keep me\n]\n");
        assert_eq!(document["a"].as_array().map(Array::len), Some(0));
    }

    #[test]
    fn makes_a_list_with_one_entry_on_each_line() {
        let mut document = DocumentMut::new();

        document["ignore"] = Item::Value(Value::Array(multi_line_array(&[
            "a".to_owned(),
            "b".to_owned(),
        ])));

        assert_eq!(
            document.to_string(),
            "ignore = [\n    \"a\",\n    \"b\",\n]\n"
        );
    }

    #[test]
    fn divides_the_text_before_a_header_at_the_last_empty_line() {
        assert_eq!(
            split_prefix("# file\n\n# about g\n"),
            ("# file\n\n", "# about g\n")
        );
        assert_eq!(split_prefix("\n"), ("\n", ""));
        assert_eq!(split_prefix("\n# section\n"), ("\n", "# section\n"));
        assert_eq!(split_prefix("# about g\n"), ("", "# about g\n"));
        assert_eq!(split_prefix("# a\n  \n"), ("# a\n  \n", ""));
        assert_eq!(split_prefix(""), ("", ""));
    }

    #[test]
    fn finds_a_comment_in_text() {
        assert!(has_comment("\n  # x\n"));
        assert!(!has_comment("\n  \n"));
    }
}
