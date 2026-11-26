//! The page Search: the file search (R14, R21) and the dictionary (R19).
//! A change of the file search updates the index at once, because the
//! `Launcher` applies the settings after each edit.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use petlauncher_config::{ConfigFile, Severity};

use super::edits::SettingsEdit;
use super::widgets::{copy, set_text, EditableList, FileErrorGroup, ListTexts, MessageRow};
use super::Context;
use crate::dictionary_window::DataProblem;

pub struct SearchPage {
    pub page: adw::PreferencesPage,
    error: FileErrorGroup,
    file_search: adw::PreferencesGroup,
    file_keyword: adw::EntryRow,
    file_keyword_message: MessageRow,
    directories: Rc<EditableList>,
    ignore: Rc<EditableList>,
    scope_error: MessageRow,
    dictionary: adw::PreferencesGroup,
    dictionary_keyword: adw::EntryRow,
    dictionary_keyword_message: MessageRow,
    wordnet_path: adw::EntryRow,
    wordnet_path_message: MessageRow,
    data_state: adw::ActionRow,
    data_icon: gtk::Image,
    copy_command: gtk::Button,
    /// The install command of the missing data, for the copy button.
    command: Rc<RefCell<String>>,
}

impl SearchPage {
    pub fn new(context: &Rc<Context>) -> Self {
        let page = adw::PreferencesPage::builder()
            .title("Search")
            .icon_name("system-search-symbolic")
            .build();
        let error = FileErrorGroup::new(context, context.file(ConfigFile::Settings));
        page.add(&error.group);

        let file_search = adw::PreferencesGroup::builder()
            .title("File search")
            .description(
                "Type the keyword, a space and a part of a file name, for example \"f notes\"",
            )
            .build();
        let file_keyword = keyword_row();
        let file_keyword_message = MessageRow::new();
        file_search.add(&file_keyword);
        file_search.add(&file_keyword_message.row);
        page.add(&file_search);

        let weak = Rc::downgrade(context);
        let save = move |edit: SettingsEdit| match weak.upgrade() {
            Some(context) => context.save_settings(edit),
            None => Ok(()),
        };
        let (add, remove) = (save.clone(), save.clone());
        let directories = EditableList::new(
            context,
            ListTexts {
                title: "Directories",
                description: "The file search finds the files below these directories",
                entry_title: "Add a directory",
            },
            true,
            move |text| add(SettingsEdit::AddDirectory(text.to_owned())),
            move |text| {
                let _ = remove(SettingsEdit::RemoveDirectory(text.to_owned()));
            },
        );
        page.add(&directories.group);

        let (add, remove) = (save.clone(), save.clone());
        let ignore = EditableList::new(
            context,
            ListTexts {
                title: "Ignore list",
                description: "The file search does not look in these. An entry is a path such as \
                     ~/tmp, a directory name such as node_modules, or a pattern such as *-wt.",
                entry_title: "Add a path, a name or a pattern",
            },
            true,
            move |text| add(SettingsEdit::AddIgnore(text.to_owned())),
            move |text| {
                let _ = remove(SettingsEdit::RemoveIgnore(text.to_owned()));
            },
        );
        let scope_error = MessageRow::new();
        ignore.group.add(&scope_error.row);
        page.add(&ignore.group);

        let dictionary = adw::PreferencesGroup::builder()
            .title("Dictionary")
            .description("Type the keyword, a space and a word, for example \"d word\"")
            .build();
        let dictionary_keyword = keyword_row();
        let dictionary_keyword_message = MessageRow::new();
        let wordnet_path = adw::EntryRow::builder()
            .title("Folder of the WordNet data")
            .show_apply_button(true)
            .build();
        let data_icon = gtk::Image::new();
        let data_state = adw::ActionRow::builder()
            .use_markup(false)
            .subtitle_selectable(true)
            .build();
        data_state.add_prefix(&data_icon);
        let copy_command = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text("Copy the Command")
            .valign(gtk::Align::Center)
            .build();
        copy_command.add_css_class("flat");
        data_state.add_suffix(&copy_command);
        dictionary.add(&dictionary_keyword);
        dictionary.add(&dictionary_keyword_message.row);
        let wordnet_path_message = MessageRow::new();
        dictionary.add(&wordnet_path);
        dictionary.add(&wordnet_path_message.row);
        dictionary.add(&data_state);
        page.add(&dictionary);

        let this = Self {
            page,
            error,
            file_search,
            file_keyword,
            file_keyword_message,
            directories,
            ignore,
            scope_error,
            dictionary,
            dictionary_keyword,
            dictionary_keyword_message,
            wordnet_path,
            wordnet_path_message,
            data_state,
            data_icon,
            copy_command,
            command: Rc::default(),
        };
        this.connect(context);
        this
    }

    fn connect(&self, context: &Rc<Context>) {
        type Edit = fn(String) -> SettingsEdit;
        let rows: [(&adw::EntryRow, &MessageRow, Edit); 3] = [
            (
                &self.file_keyword,
                &self.file_keyword_message,
                SettingsEdit::FileSearchKeyword,
            ),
            (
                &self.dictionary_keyword,
                &self.dictionary_keyword_message,
                SettingsEdit::DictionaryKeyword,
            ),
            (
                &self.wordnet_path,
                &self.wordnet_path_message,
                SettingsEdit::WordnetPath,
            ),
        ];
        for (row, message, edit) in rows {
            let weak = Rc::downgrade(context);
            let message = message.clone();
            row.connect_apply(move |row| {
                let Some(context) = weak.upgrade() else {
                    return;
                };
                match context.save_settings(edit(row.text().trim().to_owned())) {
                    Ok(()) => {
                        row.remove_css_class("error");
                        message.set(None);
                    }
                    Err(error) => {
                        row.add_css_class("error");
                        message.set(Some((Severity::Error, &error)));
                    }
                }
            });
        }
        let command = Rc::clone(&self.command);
        self.copy_command.connect_clicked(move |button| {
            copy(button, &command.borrow());
        });
    }

    pub fn reload(&self, context: &Context) {
        let settings = context.backend.settings();
        let error = context.launcher().config_error(ConfigFile::Settings);
        self.error.set("settings.toml", error.as_ref());
        let editable = error.is_none();
        self.file_search.set_sensitive(editable);
        self.dictionary.set_sensitive(editable);
        self.directories.set_editable(editable);
        self.ignore.set_editable(editable);

        set_text(&self.file_keyword, &settings.file_search.keyword);
        self.directories
            .set_items(&settings.file_search.directories);
        self.ignore.set_items(&settings.file_search.ignore);
        let scope_error = context.launcher().file_search_error().map(|error| {
            format!(
                "settings.toml: {error}. The file search uses the last correct list. \
                 Remove the entry or correct it in the file."
            )
        });
        self.scope_error
            .set(scope_error.as_deref().map(|text| (Severity::Error, text)));

        set_text(&self.dictionary_keyword, &settings.dictionary.keyword);
        set_text(&self.wordnet_path, &settings.dictionary.wordnet_path);
        match context.launcher().dictionary_suggestions("a", 1) {
            Ok(_) => {
                self.data_icon.set_icon_name(Some("emblem-ok-symbolic"));
                self.data_state.set_title("The WordNet data is found");
                self.data_state.set_subtitle("");
                self.data_state.remove_css_class("error");
                self.copy_command.set_visible(false);
            }
            Err(error) => {
                let problem = DataProblem::of(&error);
                self.data_icon.set_icon_name(Some("dialog-error-symbolic"));
                self.data_state
                    .set_title(&format!("{}. {}", problem.title, problem.cause));
                self.data_state.set_subtitle(&format!(
                    "Folder: {}\nInstall the data with: {}",
                    problem.path, problem.command
                ));
                self.data_state.add_css_class("error");
                self.command.replace(problem.command.to_owned());
                self.copy_command.set_visible(true);
            }
        }
    }
}

fn keyword_row() -> adw::EntryRow {
    adw::EntryRow::builder()
        .title("Keyword")
        .show_apply_button(true)
        .build()
}
