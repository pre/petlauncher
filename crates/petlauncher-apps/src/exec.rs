//! The `Exec` key: quoting rules and field codes of the Desktop Entry
//! Specification.

use std::path::Path;

/// Splits an `Exec` value into arguments. The value must already have the
/// escape sequences of the key file undone. Field codes stay in the
/// arguments.
pub(crate) fn split(value: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut arg = String::new();
    // `true` after a quote, so that `""` gives an empty argument.
    let mut in_arg = false;
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ' ' | '\t' | '\n' => {
                if in_arg {
                    args.push(std::mem::take(&mut arg));
                    in_arg = false;
                }
            }
            '"' => {
                in_arg = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.peek() {
                            Some(&next @ ('"' | '`' | '$' | '\\')) => {
                                arg.push(next);
                                chars.next();
                            }
                            _ => arg.push('\\'),
                        },
                        Some(other) => arg.push(other),
                        None => return Err(format!("a quote does not end in Exec: {value}")),
                    }
                }
            }
            '\'' => {
                in_arg = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(other) => arg.push(other),
                        None => return Err(format!("a quote does not end in Exec: {value}")),
                    }
                }
            }
            '\\' => {
                in_arg = true;
                if let Some(next) = chars.next() {
                    arg.push(next);
                }
            }
            _ => {
                in_arg = true;
                arg.push(c);
            }
        }
    }
    if in_arg {
        args.push(arg);
    }
    Ok(args)
}

/// The values that replace the field codes.
pub(crate) struct Fields<'a> {
    /// A file or URL for `%f`, `%F`, `%u` and `%U`.
    pub(crate) target: Option<&'a str>,
    pub(crate) name: &'a str,
    pub(crate) icon: Option<&'a str>,
    pub(crate) path: &'a Path,
}

/// Replaces the field codes in the arguments of `split`.
pub(crate) fn expand(args: &[String], fields: &Fields) -> Vec<String> {
    let mut result = Vec::with_capacity(args.len());
    for arg in args {
        match arg.as_str() {
            "%f" | "%F" | "%u" | "%U" => result.extend(fields.target.map(String::from)),
            "%i" => {
                if let Some(icon) = fields.icon {
                    result.push("--icon".to_string());
                    result.push(icon.to_string());
                }
            }
            _ => {
                if let Some(expanded) = expand_inside(arg, fields) {
                    result.push(expanded);
                }
            }
        }
    }
    result
}

/// Expands the codes in one argument. Gives `None` when the argument had
/// codes only and they expanded to nothing.
fn expand_inside(arg: &str, fields: &Fields) -> Option<String> {
    let mut result = String::with_capacity(arg.len());
    let mut had_code = false;
    let mut chars = arg.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            result.push(c);
            continue;
        }
        match chars.next() {
            Some('%') => result.push('%'),
            Some('f' | 'F' | 'u' | 'U') => result.push_str(fields.target.unwrap_or("")),
            Some('i') => result.push_str(fields.icon.unwrap_or("")),
            Some('c') => result.push_str(fields.name),
            Some('k') => result.push_str(&fields.path.to_string_lossy()),
            Some(_) => {}
            None => result.push('%'),
        }
        had_code = true;
    }
    (!(had_code && result.is_empty())).then_some(result)
}

/// The arguments from the program on: a leading `env` and its `NAME=value`
/// arguments are removed.
pub(crate) fn program_args(args: &[String]) -> &[String] {
    match args.first() {
        Some(first)
            if Path::new(first)
                .file_name()
                .is_some_and(|name| name == "env") =>
        {
            let program = args[1..]
                .iter()
                .position(|arg| !arg.contains('='))
                .map_or(args.len(), |index| index + 1);
            &args[program..]
        }
        _ => args,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields<'a>(target: Option<&'a str>, icon: Option<&'a str>) -> Fields<'a> {
        Fields {
            target,
            name: "Text Editor",
            icon,
            path: Path::new("/usr/share/applications/org.gnome.TextEditor.desktop"),
        }
    }

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn splits_at_spaces_and_ignores_extra_spaces() {
        assert_eq!(
            split("  nautilus  --new-window %U ").unwrap(),
            ["nautilus", "--new-window", "%U"]
        );
    }

    #[test]
    fn keeps_a_quoted_argument_with_spaces_as_one_argument() {
        let value = r#"/opt/microsoft/msedge/microsoft-edge "--app-url=https://x.test/?a=b c" %U"#;

        let args = split(value).unwrap();

        assert_eq!(
            args,
            [
                "/opt/microsoft/msedge/microsoft-edge",
                "--app-url=https://x.test/?a=b c",
                "%U"
            ]
        );
    }

    #[test]
    fn undoes_the_escapes_of_the_four_reserved_characters_in_quotes() {
        let args = split(r#"prog "a\"b" "c\`d" "e\$f" "g\\h""#).unwrap();

        assert_eq!(args, ["prog", "a\"b", "c`d", "e$f", "g\\h"]);
    }

    #[test]
    fn keeps_a_backslash_before_other_characters_in_quotes() {
        assert_eq!(split(r#"prog "a\nb""#).unwrap(), ["prog", "a\\nb"]);
    }

    #[test]
    fn joins_quoted_and_unquoted_parts_of_one_argument() {
        assert_eq!(
            split(r#"prog --name="My App"x"#).unwrap(),
            ["prog", "--name=My Appx"]
        );
    }

    #[test]
    fn keeps_an_empty_quoted_argument() {
        assert_eq!(split(r#"prog "" x"#).unwrap(), ["prog", "", "x"]);
    }

    #[test]
    fn reads_single_quotes_and_a_backslash_outside_quotes_as_a_shell_does() {
        assert_eq!(
            split(r"sh -c 'echo $HOME' a\ b").unwrap(),
            ["sh", "-c", "echo $HOME", "a b"]
        );
    }

    #[test]
    fn rejects_a_quote_that_does_not_end() {
        assert!(split(r#"prog "abc"#).is_err());
        assert!(split("prog 'abc").is_err());
    }

    #[test]
    fn expands_the_file_and_url_codes_to_the_target() {
        for code in ["%f", "%F", "%u", "%U"] {
            let args = strings(&["browser", code]);

            assert_eq!(
                expand(&args, &fields(Some("https://example.com/"), None)),
                ["browser", "https://example.com/"],
                "{code}"
            );
        }
    }

    #[test]
    fn removes_the_file_and_url_codes_without_a_target() {
        let args = strings(&["gnome-text-editor", "%U", "--new", "%f"]);

        assert_eq!(
            expand(&args, &fields(None, None)),
            ["gnome-text-editor", "--new"]
        );
    }

    #[test]
    fn expands_the_icon_code_to_two_arguments() {
        let args = strings(&["prog", "%i"]);

        assert_eq!(
            expand(&args, &fields(None, Some("accessories-text-editor"))),
            ["prog", "--icon", "accessories-text-editor"]
        );
        assert_eq!(expand(&args, &fields(None, None)), ["prog"]);
    }

    #[test]
    fn expands_the_name_and_file_path_codes() {
        let args = strings(&["prog", "%c", "--desktop-file=%k"]);

        assert_eq!(
            expand(&args, &fields(None, None)),
            [
                "prog",
                "Text Editor",
                "--desktop-file=/usr/share/applications/org.gnome.TextEditor.desktop"
            ]
        );
    }

    #[test]
    fn expands_a_code_inside_an_argument() {
        let args = strings(&["prog", "--open=%u", "--file=%f"]);

        assert_eq!(
            expand(&args, &fields(Some("/tmp/a b"), None)),
            ["prog", "--open=/tmp/a b", "--file=/tmp/a b"]
        );
        assert_eq!(
            expand(&args, &fields(None, None)),
            ["prog", "--open=", "--file="]
        );
    }

    #[test]
    fn writes_a_double_percent_sign_as_one_percent_sign() {
        let args = strings(&["prog", "100%%", "%%u"]);

        assert_eq!(
            expand(&args, &fields(Some("x"), None)),
            ["prog", "100%", "%u"]
        );
    }

    #[test]
    fn removes_deprecated_and_unknown_codes() {
        let args = strings(&["prog", "%d", "%D", "%n", "%N", "%v", "%m", "%z", "a%mb"]);

        assert_eq!(expand(&args, &fields(Some("x"), None)), ["prog", "ab"]);
    }

    #[test]
    fn keeps_an_empty_argument_that_has_no_code() {
        let args = strings(&["prog", ""]);

        assert_eq!(expand(&args, &fields(None, None)), ["prog", ""]);
    }

    #[test]
    fn finds_the_program_after_env_and_its_variables() {
        let args = strings(&[
            "env",
            "CODEX_HOME=/home/u/.codex",
            "LANG=C",
            "chatgpt",
            "%U",
        ]);

        assert_eq!(program_args(&args), ["chatgpt", "%U"]);
        assert_eq!(program_args(&strings(&["/usr/bin/env", "A=1", "x"])), ["x"]);
        assert_eq!(
            program_args(&strings(&["nautilus", "A=1"])),
            ["nautilus", "A=1"]
        );
        assert!(program_args(&[]).is_empty());
    }
}
