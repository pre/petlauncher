//! The page Shortcuts (R6, R7, R12): the list with add, change, remove and
//! move. A row opens the form of its shortcut as a subpage.
//!
//! `InlineShortcuts`: when `shortcuts.toml` has the shortcuts as an inline
//! list, the page shows them with the cause and a button that opens the
//! file, and it has no buttons that change the list. The window finds this
//! with an edit that changes nothing, so it writes nothing.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use petlauncher_config::{ConfigFile, EditError, Severity, Shortcut};

use super::edits::{refused, ShortcutEdit, INLINE_SHORTCUTS};
use super::form::{can_save, ShortcutForm};
use super::form_page::FormPage;
use super::widgets::{open_button, FileErrorGroup, MessageRow};
use super::Context;

pub struct ShortcutsPage {
    pub page: adw::PreferencesPage,
    error: FileErrorGroup,
    inline: adw::PreferencesGroup,
    list: adw::PreferencesGroup,
    add: gtk::Button,
    rows: RefCell<Vec<adw::ActionRow>>,
    /// The form that shows now, so it can follow a change of the list.
    form: RefCell<Weak<FormPage>>,
    context: Weak<Context>,
}

impl ShortcutsPage {
    pub fn new(context: &Rc<Context>) -> Rc<Self> {
        let page = adw::PreferencesPage::builder()
            .title("Shortcuts")
            .icon_name("user-bookmarks-symbolic")
            .build();
        let path = context.file(ConfigFile::Shortcuts);
        let error = FileErrorGroup::new(context, path.clone());
        page.add(&error.group);

        let inline = adw::PreferencesGroup::builder().visible(false).build();
        let note = MessageRow::new();
        note.set(Some((Severity::Warning, INLINE_SHORTCUTS)));
        note.row
            .add_suffix(&open_button(context, "Open File", path));
        inline.add(&note.row);
        page.add(&inline);

        let add = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add a Shortcut")
            .valign(gtk::Align::Center)
            .build();
        add.add_css_class("flat");
        let list = adw::PreferencesGroup::builder()
            .title("Shortcuts")
            .description(
                "Type the keyword of a shortcut, a space and the argument. \
                 Select a shortcut to change it.",
            )
            .header_suffix(&add)
            .build();
        page.add(&list);

        let this = Rc::new(Self {
            page,
            error,
            inline,
            list,
            add,
            rows: RefCell::default(),
            form: RefCell::default(),
            context: Rc::downgrade(context),
        });
        let weak = Rc::downgrade(&this);
        this.add.connect_clicked(move |_| {
            if let Some(page) = weak.upgrade() {
                page.open(ShortcutForm::new());
            }
        });
        this
    }

    fn open(&self, form: ShortcutForm) {
        if let Some(context) = self.context.upgrade() {
            let page = FormPage::open(&context, form);
            self.form.replace(Rc::downgrade(&page));
            // The page keeps its form until the window removes it.
            let keep = RefCell::new(Some(Rc::clone(&page)));
            page.page.connect_hidden(move |_| {
                keep.take();
            });
        }
    }

    pub fn reload(self: &Rc<Self>, context: &Context) {
        let shortcuts = context.launcher().shortcuts();
        let settings = context.backend.settings();
        let error = context.launcher().config_error(ConfigFile::Shortcuts);
        self.error.set("shortcuts.toml", error.as_ref());
        let inline =
            context.launcher().edit_shortcuts(|_| Ok(())) == Err(EditError::InlineShortcuts);
        self.inline.set_visible(inline);
        let editable = error.is_none() && !inline;
        self.add.set_sensitive(editable);
        if let Some(form) = self.form.borrow().upgrade() {
            form.retarget(&shortcuts);
        }

        for row in self.rows.take() {
            self.list.remove(&row);
        }
        let mut rows = Vec::new();
        for (index, shortcut) in shortcuts.iter().enumerate() {
            let problems = ShortcutForm::edit(index, shortcut).problems(&settings, &shortcuts);
            let row = self.row(index, shortcut, shortcuts.len(), editable);
            if !problems.is_empty() {
                let icon = gtk::Image::from_icon_name(if can_save(&problems) {
                    "dialog-warning-symbolic"
                } else {
                    "dialog-error-symbolic"
                });
                let texts: Vec<&str> = problems
                    .iter()
                    .map(|problem| problem.text.as_str())
                    .collect();
                icon.set_tooltip_text(Some(&texts.join("\n")));
                icon.add_css_class("warning");
                row.add_suffix(&icon);
            }
            self.list.add(&row);
            rows.push(row);
        }
        if shortcuts.is_empty() {
            let row = adw::ActionRow::builder()
                .title("No shortcuts. Select + to add one.")
                .build();
            row.add_css_class("dim-label");
            self.list.add(&row);
            rows.push(row);
        }
        self.rows.replace(rows);
    }

    fn row(
        self: &Rc<Self>,
        index: usize,
        shortcut: &Shortcut,
        count: usize,
        editable: bool,
    ) -> adw::ActionRow {
        let first_line = shortcut.cmd.lines().next().unwrap_or_default();
        let row = adw::ActionRow::builder()
            .title(if shortcut.name.trim().is_empty() {
                "(no name)"
            } else {
                shortcut.name.as_str()
            })
            .subtitle(format!("{}    {first_line}", shortcut.keyword))
            .subtitle_lines(1)
            .use_markup(false)
            .activatable(editable)
            .build();
        for (icon, tooltip, to) in [
            ("go-up-symbolic", "Move Up", index.checked_sub(1)),
            (
                "go-down-symbolic",
                "Move Down",
                (index + 1 < count).then_some(index + 1),
            ),
        ] {
            let button = gtk::Button::builder()
                .icon_name(icon)
                .tooltip_text(tooltip)
                .valign(gtk::Align::Center)
                .sensitive(editable && to.is_some())
                .build();
            button.add_css_class("flat");
            let weak = Rc::downgrade(self);
            button.connect_clicked(move |_| {
                if let (Some(page), Some(to)) = (weak.upgrade(), to) {
                    page.change(ShortcutEdit::Move { from: index, to });
                }
            });
            row.add_suffix(&button);
        }
        if editable {
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let weak = Rc::downgrade(self);
            let shortcut = shortcut.clone();
            row.connect_activated(move |_| {
                if let Some(page) = weak.upgrade() {
                    page.open(ShortcutForm::edit(index, &shortcut));
                }
            });
        }
        row
    }

    fn change(&self, edit: ShortcutEdit) {
        if let Some(context) = self.context.upgrade() {
            if let Err(error) = context.save_shortcuts(edit) {
                context.toast(&refused(&error));
            }
        }
    }
}
