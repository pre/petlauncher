//! The page Global Key (R4, ADR 0008). One row shows the key that GNOME has
//! for petlauncher, read again at each show of the window. Its button opens
//! the dialog of `key_dialog`, which records a new key.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use petlauncher_config::Severity;
use petlauncher_gnome::{CustomShortcuts, GnomeDesktop, SHORTCUTS_PAGE, SHORTCUT_NAME};

use super::key_dialog;
use super::widgets::MessageRow;
use super::Context;
use crate::global_key::key_label;

pub struct KeyPage {
    pub page: adw::PreferencesPage,
    row: adw::ActionRow,
    key: gtk::Label,
    unavailable: MessageRow,
}

impl KeyPage {
    pub fn new(context: &Rc<Context>) -> Self {
        let page = adw::PreferencesPage::builder()
            .title("Global Key")
            .icon_name("input-keyboard-symbolic")
            .build();
        let group = adw::PreferencesGroup::builder()
            .title("Global key")
            .description(glib::markup_escape_text(&format!(
                "The key that shows and hides petlauncher in all applications. GNOME keeps it \
                 as the custom shortcut \"{SHORTCUT_NAME}\" in {SHORTCUTS_PAGE}."
            )))
            .build();
        let unavailable = MessageRow::new();
        let key = gtk::Label::new(None);
        let button = gtk::Button::builder()
            .child(&key)
            .tooltip_text("Set Shortcut")
            .valign(gtk::Align::Center)
            .build();
        let row = adw::ActionRow::builder()
            .title("Show and hide petlauncher")
            .activatable(false)
            .build();
        row.add_suffix(&button);
        group.add(&unavailable.row);
        group.add(&row);
        page.add(&group);

        let weak = Rc::downgrade(context);
        button.connect_clicked(move |_| {
            if let Some(context) = weak.upgrade() {
                key_dialog::present(&context);
            }
        });
        Self {
            page,
            row,
            key,
            unavailable,
        }
    }

    /// Shows the key of GNOME now, so the window and GNOME Settings show the
    /// same key.
    pub fn reload(&self, context: &Context) {
        let key = match shortcuts(context) {
            Ok(shortcuts) => {
                self.unavailable.set(None);
                self.row.set_sensitive(true);
                shortcuts.global_key()
            }
            Err(error) => {
                self.unavailable.set(Some((Severity::Error, &error)));
                self.row.set_sensitive(false);
                None
            }
        };
        match key {
            Some(key) => {
                self.key.set_label(&key_label(&key));
                self.key.set_css_classes(&["keycap"]);
            }
            None => {
                self.key.set_label("Set shortcut…");
                self.key.set_css_classes(&["dim-label"]);
            }
        }
    }
}

pub(super) fn shortcuts(context: &Context) -> Result<&CustomShortcuts, String> {
    context
        .backend
        .desktop()
        .and_then(GnomeDesktop::shortcuts)
        .map_err(|error| format!("The GNOME key bindings are not available: {error}"))
}
