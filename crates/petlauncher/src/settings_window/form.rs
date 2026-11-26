//! The form of one shortcut (R6, R7, R12), with no widgets: its values, its
//! problems at the fields, and the edit that saves it.

use petlauncher_config::{validate, Browser, Problem, ProblemKind, Settings, Severity, Shortcut};

use super::edits::ShortcutEdit;

/// The browser of a shortcut (R12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserChoice {
    /// The default browser of the desktop.
    Default,
    /// An installed browser by its desktop entry ID.
    App(String),
    /// A free command. The URL is its last argument.
    Command,
}

/// Which shortcut the form saves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    New,
    /// The shortcut at `index` of the list, which had the value `original`
    /// when the form opened.
    Existing {
        index: usize,
        original: Shortcut,
    },
}

/// A field of the form with a problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Name,
    Keyword,
    Command,
    BrowserCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldProblem {
    pub field: Field,
    pub severity: Severity,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutForm {
    pub name: String,
    pub keyword: String,
    /// A URL or a script, with `%s` for the argument.
    pub cmd: String,
    /// R7: the URL or the script for a query with no argument.
    pub cmd_without_argument: String,
    pub icon: String,
    pub is_default_search: bool,
    pub run_without_argument: bool,
    pub browser: BrowserChoice,
    /// The command of `BrowserChoice::Command`.
    pub browser_command: String,
    pub target: Target,
}

impl ShortcutForm {
    /// An empty form for a new shortcut.
    pub fn new() -> Self {
        Self::from_shortcut(&Shortcut::default(), Target::New)
    }

    /// The form of the shortcut at `index`.
    pub fn edit(index: usize, shortcut: &Shortcut) -> Self {
        Self::from_shortcut(
            shortcut,
            Target::Existing {
                index,
                original: shortcut.clone(),
            },
        )
    }

    fn from_shortcut(shortcut: &Shortcut, target: Target) -> Self {
        let (browser, browser_command) = match &shortcut.browser {
            None => (BrowserChoice::Default, String::new()),
            Some(Browser::App(id)) => (BrowserChoice::App(id.clone()), String::new()),
            Some(Browser::Command(command)) => (BrowserChoice::Command, command.clone()),
        };
        Self {
            name: shortcut.name.clone(),
            keyword: shortcut.keyword.clone(),
            cmd: shortcut.cmd.clone(),
            cmd_without_argument: shortcut.cmd_without_argument.clone(),
            icon: shortcut.icon.clone(),
            is_default_search: shortcut.is_default_search,
            run_without_argument: shortcut.run_without_argument,
            browser,
            browser_command,
            target,
        }
    }

    /// The shortcut that the form saves. The spaces at the two ends of the
    /// name, the keyword and the icon go away. A command stays as typed,
    /// because a script can need its line ends.
    pub fn to_shortcut(&self) -> Shortcut {
        Shortcut {
            name: self.name.trim().to_owned(),
            keyword: self.keyword.trim().to_owned(),
            cmd: self.cmd.clone(),
            cmd_without_argument: self.cmd_without_argument.clone(),
            icon: self.icon.trim().to_owned(),
            is_default_search: self.is_default_search,
            run_without_argument: self.run_without_argument,
            browser: match &self.browser {
                BrowserChoice::Default => None,
                BrowserChoice::App(id) => Some(Browser::App(id.clone())),
                BrowserChoice::Command => {
                    Some(Browser::Command(self.browser_command.trim().to_owned()))
                }
            },
        }
    }

    /// The position of the shortcut in the list after the save.
    fn index_in(&self, shortcuts: &[Shortcut]) -> usize {
        match &self.target {
            Target::New => shortcuts.len(),
            Target::Existing { index, .. } => (*index).min(shortcuts.len()),
        }
    }

    /// The problems of the form, with the rules of `petlauncher_config` on
    /// the list after the save. An error stops the save, a warning does
    /// not.
    pub fn problems(&self, settings: &Settings, shortcuts: &[Shortcut]) -> Vec<FieldProblem> {
        let index = self.index_in(shortcuts);
        let mut list = shortcuts.to_vec();
        if index < list.len() {
            list[index] = self.to_shortcut();
        } else {
            list.push(self.to_shortcut());
        }
        let mut problems: Vec<FieldProblem> = validate(settings, &list)
            .into_iter()
            .filter(|problem| concerns(problem, index))
            .map(|problem| FieldProblem {
                field: Field::Keyword,
                severity: problem.severity(),
                text: keyword_text(&problem.kind, index),
            })
            .collect();
        if self.name.trim().is_empty() {
            problems.push(warning(
                Field::Name,
                "The name is empty, so the row has no title",
            ));
        }
        if self.cmd.trim().is_empty() && self.cmd_without_argument.trim().is_empty() {
            problems.push(warning(
                Field::Command,
                "The command is empty, so Enter does nothing",
            ));
        }
        if self.browser == BrowserChoice::Command && self.browser_command.trim().is_empty() {
            problems.push(FieldProblem {
                field: Field::BrowserCommand,
                severity: Severity::Error,
                text: "The browser command is empty".into(),
            });
        }
        problems
    }

    /// The edit that saves the form.
    pub fn save_edit(&self) -> ShortcutEdit {
        match &self.target {
            Target::New => ShortcutEdit::Add(self.to_shortcut()),
            Target::Existing { index, .. } => ShortcutEdit::Change(*index, self.to_shortcut()),
        }
    }

    /// After `EditError::FileChanged` the list can be different: the form
    /// finds its shortcut again by the value that it had when the form
    /// opened. When the shortcut is gone, the form saves a new shortcut.
    pub fn retarget(&mut self, shortcuts: &[Shortcut]) {
        if let Target::Existing { original, .. } = &self.target {
            self.target = match shortcuts.iter().position(|shortcut| shortcut == original) {
                Some(index) => Target::Existing {
                    index,
                    original: original.clone(),
                },
                None => Target::New,
            };
        }
    }
}

impl Default for ShortcutForm {
    fn default() -> Self {
        Self::new()
    }
}

/// True when the form can save: no problem is an error.
pub fn can_save(problems: &[FieldProblem]) -> bool {
    problems
        .iter()
        .all(|problem| problem.severity != Severity::Error)
}

/// The problems of one field, as one text.
pub fn field_text(problems: &[FieldProblem], field: Field) -> Option<String> {
    let texts: Vec<&str> = problems
        .iter()
        .filter(|problem| problem.field == field)
        .map(|problem| problem.text.as_str())
        .collect();
    (!texts.is_empty()).then(|| texts.join(". "))
}

fn warning(field: Field, text: &str) -> FieldProblem {
    FieldProblem {
        field,
        severity: Severity::Warning,
        text: text.into(),
    }
}

/// The text of a keyword problem in the form. The texts of
/// `petlauncher_config` name the shortcut by its number, which is not
/// necessary in the form of that shortcut.
fn keyword_text(kind: &ProblemKind, index: usize) -> String {
    match kind {
        ProblemKind::EmptyKeyword { .. } => {
            "Type a keyword: the word that starts the shortcut".into()
        }
        ProblemKind::KeywordWithSpace { .. } => {
            "A keyword cannot contain a space, because a space separates the keyword from the argument"
                .into()
        }
        ProblemKind::DuplicateKeyword {
            keyword,
            first,
            shortcut,
        } => {
            let other = if first.index == index { shortcut } else { first };
            format!("The shortcut \"{}\" has the keyword \"{keyword}\" too", other.name)
        }
        ProblemKind::KeywordOfBuiltIn {
            keyword, built_in, ..
        } => format!("\"{keyword}\" is also the keyword of {built_in}"),
        other => other.to_string(),
    }
}

/// True when the problem is about the shortcut at `index`.
fn concerns(problem: &Problem, index: usize) -> bool {
    match &problem.kind {
        ProblemKind::EmptyKeyword { owner } | ProblemKind::KeywordWithSpace { owner, .. } => {
            matches!(owner, petlauncher_config::validate::KeywordOwner::Shortcut(shortcut) if shortcut.index == index)
        }
        ProblemKind::DuplicateKeyword {
            first, shortcut, ..
        } => first.index == index || shortcut.index == index,
        ProblemKind::KeywordOfBuiltIn { shortcut, .. } => shortcut.index == index,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shortcut(name: &str, keyword: &str, cmd: &str) -> Shortcut {
        Shortcut {
            name: name.into(),
            keyword: keyword.into(),
            cmd: cmd.into(),
            ..Shortcut::default()
        }
    }

    fn list() -> Vec<Shortcut> {
        vec![
            shortcut("Search", "g", "https://example.com/?q=%s"),
            shortcut(
                "Rektangel",
                "rektangel",
                "https://github.com/pre/rektangel/pull/%s",
            ),
        ]
    }

    fn filled(keyword: &str) -> ShortcutForm {
        ShortcutForm {
            name: "Jira".into(),
            keyword: keyword.into(),
            cmd: "https://jira.example.com/browse/%s".into(),
            ..ShortcutForm::new()
        }
    }

    #[test]
    fn a_new_form_saves_a_new_shortcut_with_the_typed_values() {
        let mut form = filled(" jira ");
        form.cmd_without_argument = "https://jira.example.com/".into();
        form.run_without_argument = true;

        let edit = form.save_edit();

        let ShortcutEdit::Add(saved) = edit else {
            panic!("{edit:?}");
        };
        assert_eq!(saved.keyword, "jira");
        assert_eq!(saved.cmd_without_argument, "https://jira.example.com/");
        assert!(saved.run_without_argument);
        assert_eq!(saved.browser, None);
    }

    #[test]
    fn an_edited_shortcut_keeps_its_values_and_saves_a_change() {
        let mut original = shortcut("Beacon", "beacon", "xdg-open \"$1\"\nexit 0\n");
        original.browser = Some(Browser::Command("firefox -P work".into()));
        original.is_default_search = true;

        let form = ShortcutForm::edit(1, &original);

        assert_eq!(form.browser, BrowserChoice::Command);
        assert_eq!(form.browser_command, "firefox -P work");
        assert_eq!(form.to_shortcut(), original);
        assert_eq!(form.save_edit(), ShortcutEdit::Change(1, original));
    }

    #[test]
    fn an_installed_browser_is_saved_by_its_id() {
        let mut form = filled("edge");
        form.browser = BrowserChoice::App("microsoft-edge.desktop".into());

        assert_eq!(
            form.to_shortcut().browser,
            Some(Browser::App("microsoft-edge.desktop".into()))
        );
    }

    #[test]
    fn an_empty_keyword_and_a_keyword_with_a_space_are_errors_at_the_keyword() {
        for keyword in ["", "ji ra"] {
            let problems = filled(keyword).problems(&Settings::default(), &list());

            assert!(!can_save(&problems), "{keyword:?}");
            assert!(field_text(&problems, Field::Keyword).is_some());
        }
    }

    #[test]
    fn a_keyword_of_a_different_shortcut_is_a_warning_and_the_save_goes_on() {
        let problems = filled("g").problems(&Settings::default(), &list());

        assert!(can_save(&problems));
        let text = field_text(&problems, Field::Keyword).unwrap();
        assert_eq!(text, "The shortcut \"Search\" has the keyword \"g\" too");
    }

    #[test]
    fn the_first_of_two_shortcuts_with_a_keyword_names_the_second() {
        let form = ShortcutForm::edit(0, &list()[0]);
        let mut shortcuts = list();
        shortcuts.push(shortcut("Later", "g", "x"));

        let problems = form.problems(&Settings::default(), &shortcuts);

        assert_eq!(
            field_text(&problems, Field::Keyword).unwrap(),
            "The shortcut \"Later\" has the keyword \"g\" too"
        );
    }

    #[test]
    fn the_keyword_errors_have_texts_for_the_form() {
        let empty = filled("").problems(&Settings::default(), &list());
        let space = filled("ji ra").problems(&Settings::default(), &list());

        assert_eq!(
            field_text(&empty, Field::Keyword).unwrap(),
            "Type a keyword: the word that starts the shortcut"
        );
        assert!(field_text(&space, Field::Keyword)
            .unwrap()
            .starts_with("A keyword cannot contain a space"));
    }

    #[test]
    fn the_keyword_of_the_file_search_is_a_warning() {
        let problems = filled("f").problems(&Settings::default(), &list());

        assert!(can_save(&problems));
        assert_eq!(
            field_text(&problems, Field::Keyword).unwrap(),
            "\"f\" is also the keyword of the file search"
        );
    }

    #[test]
    fn an_edited_shortcut_is_not_a_duplicate_of_itself() {
        let form = ShortcutForm::edit(1, &list()[1]);

        let problems = form.problems(&Settings::default(), &list());

        assert_eq!(problems, []);
    }

    #[test]
    fn a_problem_of_a_different_shortcut_does_not_show_in_the_form() {
        let mut shortcuts = list();
        shortcuts.push(shortcut("Other", "rektangel", "x"));

        let problems = filled("jira").problems(&Settings::default(), &shortcuts);

        assert_eq!(problems, []);
    }

    #[test]
    fn an_empty_browser_command_is_an_error_and_an_empty_name_a_warning() {
        let mut form = filled("jira");
        form.name.clear();
        form.browser = BrowserChoice::Command;

        let problems = form.problems(&Settings::default(), &list());

        assert!(!can_save(&problems));
        assert!(field_text(&problems, Field::BrowserCommand).is_some());
        assert!(field_text(&problems, Field::Name).is_some());
    }

    #[test]
    fn an_empty_command_is_a_warning() {
        let mut form = filled("jira");
        form.cmd.clear();

        let problems = form.problems(&Settings::default(), &list());

        assert!(can_save(&problems));
        assert!(field_text(&problems, Field::Command).is_some());
    }

    #[test]
    fn after_a_change_in_an_editor_the_form_finds_its_shortcut_again() {
        let mut form = ShortcutForm::edit(1, &list()[1]);
        form.keyword = "spr".into();
        let mut changed = list();
        changed.insert(0, shortcut("New first", "n", "x"));

        form.retarget(&changed);

        assert_eq!(
            form.save_edit(),
            ShortcutEdit::Change(2, {
                let mut saved = list()[1].clone();
                saved.keyword = "spr".into();
                saved
            })
        );
    }

    #[test]
    fn after_a_change_in_an_editor_a_shortcut_that_is_gone_is_saved_as_new() {
        let mut form = ShortcutForm::edit(1, &list()[1]);
        let changed = vec![list()[0].clone()];

        form.retarget(&changed);

        assert_eq!(form.target, Target::New);
        assert!(matches!(form.save_edit(), ShortcutEdit::Add(_)));
    }
}
