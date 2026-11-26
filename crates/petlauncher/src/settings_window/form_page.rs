//! The subpage that adds or changes one shortcut (R6, R7, R12). The values
//! are in a `ShortcutForm`; the page shows its problems at the fields. An
//! error stops the save, a warning does not. When the store refuses the
//! save, the page stays open with the typed values and shows the cause.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use petlauncher_config::{EditError, Severity};

use super::edits::{refused, ShortcutEdit};
use super::form::{can_save, field_text, BrowserChoice, Field, FieldProblem, ShortcutForm, Target};
use super::widgets::{script_view, text_of, MessageRow};
use super::Context;
use crate::log;

pub struct FormPage {
    pub page: adw::NavigationPage,
    form: RefCell<ShortcutForm>,
    /// A new form shows its problems only after the first change, so an
    /// empty form does not start with warnings.
    edited: Cell<bool>,
    context: std::rc::Weak<Context>,
    save: gtk::Button,
    refusal: MessageRow,
    name: adw::EntryRow,
    name_message: MessageRow,
    keyword: adw::EntryRow,
    keyword_message: MessageRow,
    icon: adw::EntryRow,
    icon_preview: gtk::Image,
    cmd: gtk::TextView,
    cmd_message: MessageRow,
    cmd_without_argument: gtk::TextView,
    is_default_search: adw::SwitchRow,
    run_without_argument: adw::SwitchRow,
    browser: adw::ComboRow,
    /// The choice of each line of the browser list.
    browser_choices: Vec<BrowserChoice>,
    browser_command: adw::EntryRow,
    browser_command_message: MessageRow,
}

impl FormPage {
    /// Shows the form as a subpage of the window.
    pub fn open(context: &Rc<Context>, form: ShortcutForm) -> Rc<Self> {
        let title = match form.target {
            Target::New => "New Shortcut",
            Target::Existing { .. } => "Shortcut",
        };
        let content = adw::PreferencesPage::new();
        let refusal = MessageRow::new();
        let top = message_group(&refusal);
        content.add(&top);

        let main = adw::PreferencesGroup::new();
        let name = adw::EntryRow::builder().title("Name").build();
        let name_message = MessageRow::new();
        let keyword = adw::EntryRow::builder().title("Keyword").build();
        let keyword_message = MessageRow::new();
        let icon_preview = gtk::Image::builder().pixel_size(24).build();
        let icon = adw::EntryRow::builder()
            .title("Icon: an icon name or the path of an image")
            .build();
        icon.add_suffix(&icon_preview);
        main.add(&name);
        main.add(&name_message.row);
        main.add(&keyword);
        main.add(&keyword_message.row);
        main.add(&icon);
        content.add(&main);

        let (cmd, cmd_box) = script_view("Command");
        let cmd_message = MessageRow::new();
        let command = adw::PreferencesGroup::builder()
            .title("Command")
            .description(
                "A URL or a script. %s in it becomes the argument. A URL opens in the browser. \
                 A script runs with the argument also as $1.",
            )
            .build();
        command.add(&cmd_box);
        let command_messages = message_group(&cmd_message);
        content.add(&command);
        content.add(&command_messages);

        let (cmd_without_argument, without_box) = script_view("Command without argument");
        let without = adw::PreferencesGroup::builder()
            .title("Command without argument")
            .description(
                "A URL or a script for the keyword with no argument. When this is empty, the \
                 switch \"Run without argument\" below decides.",
            )
            .build();
        without.add(&without_box);
        content.add(&without);

        let options = adw::PreferencesGroup::builder().title("Options").build();
        let is_default_search = adw::SwitchRow::builder()
            .title("Default search")
            .subtitle("When nothing matches the query, show this shortcut with the full query as its argument")
            .build();
        let run_without_argument = adw::SwitchRow::builder()
            .title("Run without argument")
            .subtitle(
                "Run the command also with no argument. Off: Enter with no argument does nothing.",
            )
            .build();
        options.add(&is_default_search);
        options.add(&run_without_argument);
        content.add(&options);

        let (browser, browser_choices) = browser_row(context, &form.browser);
        let browser_command = adw::EntryRow::builder()
            .title("Browser command, the URL is its last argument")
            .build();
        let browser_command_message = MessageRow::new();
        let browsers = adw::PreferencesGroup::builder()
            .title("Browser")
            .description("The browser that opens a URL of this shortcut")
            .build();
        browsers.add(&browser);
        browsers.add(&browser_command);
        browsers.add(&browser_command_message.row);
        content.add(&browsers);

        let is_existing = matches!(form.target, Target::Existing { .. });
        let remove = gtk::Button::builder()
            .label("Remove Shortcut")
            .halign(gtk::Align::Center)
            .build();
        remove.add_css_class("destructive-action");
        remove.add_css_class("pill");
        if is_existing {
            let group = adw::PreferencesGroup::new();
            group.add(&remove);
            content.add(&group);
        }

        let save = gtk::Button::builder().label("Save").build();
        save.add_css_class("suggested-action");
        let header = adw::HeaderBar::new();
        header.pack_end(&save);
        let view = adw::ToolbarView::builder().content(&content).build();
        view.add_top_bar(&header);
        let page = adw::NavigationPage::builder()
            .title(title)
            .child(&view)
            .build();

        let this = Rc::new(Self {
            page,
            form: RefCell::new(form),
            edited: Cell::new(false),
            context: Rc::downgrade(context),
            save,
            refusal,
            name,
            name_message,
            keyword,
            keyword_message,
            icon,
            icon_preview,
            cmd,
            cmd_message,
            cmd_without_argument,
            is_default_search,
            run_without_argument,
            browser,
            browser_choices,
            browser_command,
            browser_command_message,
        });
        this.show_values();
        this.connect(&remove);
        this.update();
        log::debug(format_args!(
            "settings window: form, {:?}",
            this.form.borrow().target
        ));
        context.window.push_subpage(&this.page);
        this.name.grab_focus();
        this
    }

    fn show_values(&self) {
        let form = self.form.borrow();
        self.name.set_text(&form.name);
        self.keyword.set_text(&form.keyword);
        self.icon.set_text(&form.icon);
        self.cmd.buffer().set_text(&form.cmd);
        self.cmd_without_argument
            .buffer()
            .set_text(&form.cmd_without_argument);
        self.is_default_search.set_active(form.is_default_search);
        self.run_without_argument
            .set_active(form.run_without_argument);
        let position = self
            .browser_choices
            .iter()
            .position(|choice| *choice == form.browser)
            .unwrap_or(0);
        self.browser.set_selected(position as u32);
        self.browser_command.set_text(&form.browser_command);
    }

    fn connect(self: &Rc<Self>, remove: &gtk::Button) {
        let weak = Rc::downgrade(self);
        let changed = move || {
            if let Some(page) = weak.upgrade() {
                page.edited.set(true);
                page.update();
            }
        };
        for entry in [&self.name, &self.keyword, &self.icon, &self.browser_command] {
            let changed = changed.clone();
            entry.connect_changed(move |_| changed());
        }
        for view in [&self.cmd, &self.cmd_without_argument] {
            let changed = changed.clone();
            view.buffer().connect_changed(move |_| changed());
        }
        for switch in [&self.is_default_search, &self.run_without_argument] {
            let changed = changed.clone();
            switch.connect_active_notify(move |_| changed());
        }
        self.browser.connect_selected_notify(move |_| changed());

        let weak = Rc::downgrade(self);
        self.save.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.save();
            }
        });
        let weak = Rc::downgrade(self);
        remove.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.confirm_remove();
            }
        });
    }

    /// Reads the widgets into the form and shows its problems.
    fn update(&self) {
        {
            let mut form = self.form.borrow_mut();
            form.name = self.name.text().to_string();
            form.keyword = self.keyword.text().to_string();
            form.icon = self.icon.text().to_string();
            form.cmd = text_of(&self.cmd);
            form.cmd_without_argument = text_of(&self.cmd_without_argument);
            form.is_default_search = self.is_default_search.is_active();
            form.run_without_argument = self.run_without_argument.is_active();
            form.browser = self
                .browser_choices
                .get(self.browser.selected() as usize)
                .cloned()
                .unwrap_or(BrowserChoice::Default);
            form.browser_command = self.browser_command.text().to_string();
        }
        let form = self.form.borrow();
        self.browser_command
            .set_visible(form.browser == BrowserChoice::Command);
        set_icon(&self.icon_preview, form.icon.trim());
        let Some(context) = self.context.upgrade() else {
            return;
        };
        let problems = form.problems(&context.backend.settings(), &context.launcher().shortcuts());
        let shown = match form.target {
            Target::New if !self.edited.get() => &[][..],
            _ => &problems[..],
        };
        for (field, row, message) in [
            (
                Field::Name,
                self.name.upcast_ref::<gtk::Widget>(),
                &self.name_message,
            ),
            (
                Field::Keyword,
                self.keyword.upcast_ref(),
                &self.keyword_message,
            ),
            (Field::Command, self.cmd.upcast_ref(), &self.cmd_message),
            (
                Field::BrowserCommand,
                self.browser_command.upcast_ref(),
                &self.browser_command_message,
            ),
        ] {
            show_problem(shown, field, row, message);
        }
        self.save.set_sensitive(can_save(&problems));
    }

    fn save(&self) {
        let Some(context) = self.context.upgrade() else {
            return;
        };
        let edit = self.form.borrow().save_edit();
        let result = context.save_shortcuts(edit);
        self.after(&context, result, "Saved the shortcut");
    }

    fn confirm_remove(self: &Rc<Self>) {
        let Some(context) = self.context.upgrade() else {
            return;
        };
        let name = self.form.borrow().name.clone();
        let dialog = adw::MessageDialog::builder()
            .transient_for(&context.window)
            .modal(true)
            .heading("Remove the shortcut?")
            .body(format!(
                "The shortcut \"{name}\" and the comment lines above it go away from shortcuts.toml."
            ))
            .build();
        dialog.add_responses(&[("cancel", "Cancel"), ("remove", "Remove")]);
        dialog.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        dialog.connect_response(None, move |_, response| {
            let (Some(page), "remove") = (weak.upgrade(), response) else {
                return;
            };
            let Some(context) = page.context.upgrade() else {
                return;
            };
            let Target::Existing { index, .. } = page.form.borrow().target.clone() else {
                return;
            };
            let result = context.save_shortcuts(ShortcutEdit::Remove(index));
            page.after(&context, result, "Removed the shortcut");
        });
        log::debug("settings window: remove dialog");
        dialog.present();
    }

    /// After a save: close the page, or show why the store refused it. The
    /// typed values stay.
    fn after(&self, context: &Context, result: Result<(), EditError>, done: &str) {
        match result {
            Ok(()) => {
                context.window.pop_subpage();
                context.toast(done);
            }
            Err(error) => {
                if matches!(error, EditError::FileChanged | EditError::NoShortcut { .. }) {
                    self.retarget(&context.launcher().shortcuts());
                }
                let text = match error {
                    EditError::File(_) => format!(
                        "{} The values in this form stay until you close it.",
                        refused(&error)
                    ),
                    _ => refused(&error),
                };
                self.refusal.set(Some((Severity::Error, &text)));
                self.update();
            }
        }
    }

    /// The list changed: the form finds its shortcut again.
    pub fn retarget(&self, shortcuts: &[petlauncher_config::Shortcut]) {
        self.form.borrow_mut().retarget(shortcuts);
        if self.form.borrow().target == Target::New {
            self.page.set_title("New Shortcut");
        }
        self.update();
    }
}

/// The browser list: the default browser, the installed browsers by name,
/// and a free command. A browser of the shortcut that is not installed is
/// in the list too, so a save keeps it.
fn browser_row(context: &Context, current: &BrowserChoice) -> (adw::ComboRow, Vec<BrowserChoice>) {
    let mut labels = vec!["Default browser".to_owned()];
    let mut choices = vec![BrowserChoice::Default];
    for entry in context.launcher().browsers() {
        labels.push(entry.name.clone());
        choices.push(BrowserChoice::App(entry.id.clone()));
    }
    if let BrowserChoice::App(id) = current {
        if !choices.contains(current) {
            labels.push(format!("{id} (not installed)"));
            choices.push(current.clone());
        }
    }
    labels.push("A command".to_owned());
    choices.push(BrowserChoice::Command);
    let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
    let row = adw::ComboRow::builder()
        .title("Open URLs with")
        .model(&gtk::StringList::new(&labels))
        .build();
    (row, choices)
}

fn show_problem(
    problems: &[FieldProblem],
    field: Field,
    widget: &gtk::Widget,
    message: &MessageRow,
) {
    let severity = problems
        .iter()
        .filter(|problem| problem.field == field)
        .map(|problem| problem.severity)
        .max_by_key(|severity| *severity == Severity::Error);
    widget.remove_css_class("error");
    widget.remove_css_class("warning");
    match (severity, field_text(problems, field)) {
        (Some(severity), Some(text)) => {
            widget.add_css_class(match severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            });
            message.set(Some((severity, &text)));
        }
        _ => message.set(None),
    }
}

/// A group with only a message row. It shows only with the row, because an
/// empty group shows as a line.
fn message_group(message: &MessageRow) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.add(&message.row);
    message
        .row
        .bind_property("visible", &group, "visible")
        .sync_create()
        .build();
    group
}

/// The preview of the icon. A shortcut with no icon has no icon in the
/// launcher too.
fn set_icon(image: &gtk::Image, icon: &str) {
    image.set_visible(!icon.is_empty());
    if icon.is_empty() {
        image.clear();
    } else if icon.starts_with('/') || icon.starts_with('~') {
        let path = match icon.strip_prefix("~/") {
            Some(rest) => gtk::glib::home_dir().join(rest),
            None => icon.into(),
        };
        image.set_from_file(Some(path));
    } else {
        image.set_icon_name(Some(icon));
    }
}
