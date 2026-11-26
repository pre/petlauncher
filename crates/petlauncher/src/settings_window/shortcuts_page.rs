//! The page Shortcuts (R6, R7, R12): the list with add, change, remove and
//! move. A row opens the form of its shortcut as a subpage. The mouse drags
//! a row by the handle at its right edge to a new place, and Alt+Up and
//! Alt+Down move the focused row.
//!
//! `InlineShortcuts`: when `shortcuts.toml` has the shortcuts as an inline
//! list, the page shows them with the cause and a button that opens the
//! file, and it has no buttons that change the list. The window finds this
//! with an edit that changes nothing, so it writes nothing.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib, graphene};
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
    /// The row that gets the focus after the next reload: the place of a
    /// shortcut after Alt+Up or Alt+Down.
    focus_after_reload: Cell<Option<usize>>,
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
            focus_after_reload: Cell::default(),
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
            let row = self.row(index, shortcut, editable);
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
            if editable {
                row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                self.make_movable(&row, index, shortcuts.len());
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
        if let Some(row) = self
            .focus_after_reload
            .take()
            .and_then(|index| rows.get(index))
        {
            row.grab_focus();
        }
        self.rows.replace(rows);
    }

    fn row(self: &Rc<Self>, index: usize, shortcut: &Shortcut, editable: bool) -> adw::ActionRow {
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
        if editable {
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

    /// The handle at the right edge of the row, the drop on the row, and
    /// Alt+Up and Alt+Down. Each move is `ShortcutEdit::Move`, so the
    /// comments of the shortcut move with it.
    fn make_movable(self: &Rc<Self>, row: &adw::ActionRow, index: usize, count: usize) {
        let handle = gtk::Image::builder()
            .icon_name("list-drag-handle-symbolic")
            .tooltip_text("Drag to move the shortcut, or press Alt+Up or Alt+Down")
            .build();
        handle.set_cursor_from_name(Some("grab"));
        handle.update_property(&[gtk::accessible::Property::Label("Drag to Move")]);
        let drag = gtk::DragSource::builder()
            .actions(gdk::DragAction::MOVE)
            .build();
        let (weak_row, weak_handle) = (row.downgrade(), handle.downgrade());
        drag.connect_prepare(move |drag, x, y| {
            let (row, handle) = (weak_row.upgrade()?, weak_handle.upgrade()?);
            let point = handle.compute_point(&row, &graphene::Point::new(x as f32, y as f32))?;
            // The image of the row has the background of the row only
            // during the drag. In the list, the list draws the background.
            row.add_css_class("dragged");
            drag.set_icon(
                Some(&gtk::WidgetPaintable::new(Some(&row))),
                point.x() as i32,
                point.y() as i32,
            );
            Some(gdk::ContentProvider::for_value(&(index as u32).to_value()))
        });
        let weak_row = row.downgrade();
        drag.connect_drag_end(move |_, _, _| {
            if let Some(row) = weak_row.upgrade() {
                row.remove_css_class("dragged");
            }
        });
        // The click gesture takes the press from the list, so a click on
        // the handle does not open the form. The drag shares the press.
        let click = gtk::GestureClick::new();
        click.connect_pressed(|click, _, _, _| {
            click.set_state(gtk::EventSequenceState::Claimed);
        });
        click.group_with(&drag);
        handle.add_controller(drag);
        handle.add_controller(click);
        row.add_suffix(&handle);

        let target = gtk::DropTarget::new(u32::static_type(), gdk::DragAction::MOVE);
        let weak_row = row.downgrade();
        target.connect_motion(move |_, _, y| {
            if let Some(row) = weak_row.upgrade() {
                mark_drop(&row, Some(lower_half(&row, y)));
            }
            gdk::DragAction::MOVE
        });
        let weak_row = row.downgrade();
        target.connect_leave(move |_| {
            if let Some(row) = weak_row.upgrade() {
                mark_drop(&row, None);
            }
        });
        let (weak, weak_row) = (Rc::downgrade(self), row.downgrade());
        target.connect_drop(move |_, value, _, y| {
            let (Some(page), Some(row), Ok(from)) =
                (weak.upgrade(), weak_row.upgrade(), value.get::<u32>())
            else {
                return false;
            };
            mark_drop(&row, None);
            if let Some(edit) = ShortcutEdit::drop(from as usize, index, lower_half(&row, y)) {
                page.change(edit);
            }
            true
        });
        row.add_controller(target);

        let keys = gtk::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, keyval, _, modifiers| {
            let up = match keyval {
                gdk::Key::Up => true,
                gdk::Key::Down => false,
                _ => return glib::Propagation::Proceed,
            };
            if modifiers & gtk::accelerator_get_default_mod_mask() != gdk::ModifierType::ALT_MASK {
                return glib::Propagation::Proceed;
            }
            if let (Some(page), Some(edit)) = (weak.upgrade(), ShortcutEdit::step(index, up, count))
            {
                if let ShortcutEdit::Move { to, .. } = edit {
                    page.focus_after_reload.set(Some(to));
                }
                page.change(edit);
            }
            glib::Propagation::Stop
        });
        row.add_controller(keys);
    }

    fn change(&self, edit: ShortcutEdit) {
        if let Some(context) = self.context.upgrade() {
            if let Err(error) = context.save_shortcuts(edit) {
                context.toast(&refused(&error));
            }
        }
    }
}

fn lower_half(row: &adw::ActionRow, y: f64) -> bool {
    y > f64::from(row.height()) / 2.0
}

/// Shows where a dropped shortcut goes: a line above or below the row.
fn mark_drop(row: &adw::ActionRow, lower_half: Option<bool>) {
    row.remove_css_class("drop-above");
    row.remove_css_class("drop-below");
    if let Some(lower_half) = lower_half {
        row.add_css_class(if lower_half {
            "drop-below"
        } else {
            "drop-above"
        });
    }
}
