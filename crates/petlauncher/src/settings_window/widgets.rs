//! Widgets that the pages share. The rows show text from the files and from
//! the owner, so no row uses Pango markup.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use adw::prelude::*;
use gtk::gio;
use petlauncher_config::{ConfigError, Severity};

use super::edits::tilde;
use super::Context;
use crate::log;

/// A row with an icon and a text: an error, a warning or a note. It hides
/// when it has no text.
#[derive(Clone)]
pub struct MessageRow {
    pub row: adw::ActionRow,
    icon: gtk::Image,
}

impl MessageRow {
    pub fn new() -> Self {
        let icon = gtk::Image::new();
        let row = adw::ActionRow::builder()
            .use_markup(false)
            .activatable(false)
            .visible(false)
            .build();
        row.add_css_class("message");
        row.add_prefix(&icon);
        Self { row, icon }
    }

    pub fn set(&self, message: Option<(Severity, &str)>) {
        self.row.remove_css_class("error");
        self.row.remove_css_class("warning");
        let Some((severity, text)) = message else {
            self.row.set_visible(false);
            return;
        };
        let (class, icon) = match severity {
            Severity::Error => ("error", "dialog-error-symbolic"),
            Severity::Warning => ("warning", "dialog-warning-symbolic"),
        };
        self.row.add_css_class(class);
        self.icon.set_icon_name(Some(icon));
        self.row.set_title(text);
        self.row.set_visible(true);
    }
}

/// The group at the top of a page that shows the error of a file (R18): the
/// old values stay in effect, and the window does not change the file.
pub struct FileErrorGroup {
    pub group: adw::PreferencesGroup,
    row: adw::ActionRow,
}

impl FileErrorGroup {
    pub fn new(context: &Rc<Context>, path: PathBuf) -> Self {
        let row = adw::ActionRow::builder()
            .use_markup(false)
            .subtitle_selectable(true)
            .build();
        row.add_css_class("error");
        row.add_prefix(&gtk::Image::from_icon_name("dialog-error-symbolic"));
        row.add_suffix(&open_button(context, "Open File", path));
        let group = adw::PreferencesGroup::builder().visible(false).build();
        group.add(&row);
        Self { group, row }
    }

    /// Shows the error, or hides the group.
    pub fn set(&self, file_name: &str, error: Option<&ConfigError>) {
        match error {
            Some(error) => {
                self.row.set_title(&format!(
                    "{file_name} has an error, so its old values stay in effect. \
                     The window does not change the file until the error is corrected."
                ));
                self.row.set_subtitle(&error.to_string());
                self.group.set_visible(true);
            }
            None => self.group.set_visible(false),
        }
    }
}

/// A button that opens a file or a folder with its default application.
pub fn open_button(context: &Rc<Context>, label: &str, path: PathBuf) -> gtk::Button {
    let button = gtk::Button::builder()
        .label(label)
        .valign(gtk::Align::Center)
        .build();
    let context = Rc::downgrade(context);
    button.connect_clicked(move |_| {
        if let Some(context) = context.upgrade() {
            open_path(&context, &path);
        }
    });
    button
}

/// Opens `path` with its default application: a text file in the text
/// editor, a directory in the file manager.
pub fn open_path(context: &Rc<Context>, path: &Path) {
    let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(path)));
    let weak = Rc::downgrade(context);
    let shown = path.display().to_string();
    launcher.launch(
        Some(&context.window),
        gio::Cancellable::NONE,
        move |result| {
            if let Err(error) = result {
                log::info(format_args!(
                    "settings window: cannot open {shown}: {error}"
                ));
                if let Some(context) = weak.upgrade() {
                    context.toast(&format!("{shown} cannot be opened: {error}"));
                }
            }
        },
    );
}

/// Adds a text to a list. The error text is for the owner.
type Add = Rc<dyn Fn(&str) -> Result<(), String>>;

/// A list of texts with a remove button on each row, and an entry row that
/// adds a text. With `folders`, a button selects a folder for the entry.
pub struct EditableList {
    pub group: adw::PreferencesGroup,
    pub entry: adw::EntryRow,
    pub message: MessageRow,
    rows: RefCell<Vec<adw::ActionRow>>,
    remove: Rc<dyn Fn(&str)>,
}

pub struct ListTexts<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub entry_title: &'a str,
}

impl EditableList {
    pub fn new(
        context: &Rc<Context>,
        texts: ListTexts,
        folders: bool,
        add: impl Fn(&str) -> Result<(), String> + 'static,
        remove: impl Fn(&str) + 'static,
    ) -> Rc<Self> {
        let group = adw::PreferencesGroup::builder()
            .title(texts.title)
            .description(texts.description)
            .build();
        let entry = adw::EntryRow::builder()
            .title(texts.entry_title)
            .show_apply_button(true)
            .build();
        let message = MessageRow::new();
        group.add(&entry);
        group.add(&message.row);
        let list = Rc::new(Self {
            group,
            entry,
            message,
            rows: RefCell::default(),
            remove: Rc::new(remove),
        });
        let add: Add = Rc::new(add);
        let weak = Rc::downgrade(&list);
        let on_apply = Rc::clone(&add);
        list.entry.connect_apply(move |entry| {
            if let Some(list) = weak.upgrade() {
                list.add(&on_apply, &entry.text());
            }
        });
        let weak = Rc::downgrade(&list);
        list.entry.connect_changed(move |entry| {
            if let Some(list) = weak.upgrade() {
                entry.remove_css_class("error");
                list.message.set(None);
            }
        });
        if folders {
            let button = gtk::Button::builder()
                .icon_name("folder-open-symbolic")
                .tooltip_text("Select a Folder")
                .valign(gtk::Align::Center)
                .build();
            button.add_css_class("flat");
            let weak = Rc::downgrade(&list);
            let window = context.window.clone();
            let home = context.home();
            button.connect_clicked(move |_| {
                let weak = weak.clone();
                let add = Rc::clone(&add);
                let home = home.clone();
                gtk::FileDialog::builder()
                    .title("Select a Folder")
                    .modal(true)
                    .build()
                    .select_folder(Some(&window), gio::Cancellable::NONE, move |result| {
                        let (Ok(folder), Some(list)) = (result, weak.upgrade()) else {
                            return;
                        };
                        if let Some(path) = folder.path() {
                            let text = tilde(&path, &home);
                            list.entry.set_text(&text);
                            list.add(&add, &text);
                        }
                    });
            });
            list.entry.add_suffix(&button);
        }
        list
    }

    fn add(&self, add: &Add, text: &str) {
        match add(text.trim()) {
            Ok(()) => {
                self.entry.set_text("");
                self.message.set(None);
            }
            Err(error) => {
                self.entry.add_css_class("error");
                self.message.set(Some((Severity::Error, &error)));
            }
        }
    }

    /// Shows `items` above the entry row.
    pub fn set_items(&self, items: &[String]) {
        for row in self.rows.take() {
            self.group.remove(&row);
        }
        let Some(list_box) = self.entry.parent().and_downcast::<gtk::ListBox>() else {
            return;
        };
        let mut rows = Vec::new();
        for (position, item) in items.iter().enumerate() {
            let row = adw::ActionRow::builder()
                .title(item.as_str())
                .use_markup(false)
                .build();
            let button = gtk::Button::builder()
                .icon_name("user-trash-symbolic")
                .tooltip_text("Remove")
                .valign(gtk::Align::Center)
                .build();
            button.add_css_class("flat");
            let remove = Rc::clone(&self.remove);
            let item = item.clone();
            button.connect_clicked(move |_| remove(&item));
            row.add_suffix(&button);
            list_box.insert(&row, position as i32);
            rows.push(row);
        }
        self.rows.replace(rows);
    }

    /// While the file has an error, the list and the entry do not change.
    pub fn set_editable(&self, editable: bool) {
        self.group.set_sensitive(editable);
    }
}

/// Sets the text of an entry only when it is different, so the cursor of
/// the owner stays.
pub fn set_text(entry: &impl IsA<gtk::Editable>, text: &str) {
    if entry.text() != text {
        entry.set_text(text);
    }
}

/// A plain text field for a script, with a fixed-width font. `label` is
/// its name for a screen reader.
pub fn script_view(label: &str) -> (gtk::TextView, gtk::ScrolledWindow) {
    let view = gtk::TextView::builder()
        .monospace(true)
        .wrap_mode(gtk::WrapMode::WordChar)
        .top_margin(8)
        .bottom_margin(8)
        .left_margin(10)
        .right_margin(10)
        .accepts_tab(false)
        .build();
    view.update_property(&[gtk::accessible::Property::Label(label)]);
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&view)
        .min_content_height(72)
        .max_content_height(220)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    scrolled.add_css_class("card");
    scrolled.set_overflow(gtk::Overflow::Hidden);
    (view, scrolled)
}

pub fn text_of(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), false)
        .to_string()
}

/// Copies `text` to the clipboard.
pub fn copy(widget: &impl IsA<gtk::Widget>, text: &str) {
    widget.as_ref().clipboard().set_text(text);
}
