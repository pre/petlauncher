//! The page Global Key (R4, ADR 0008). It shows the key that GNOME has for
//! petlauncher, read again each time the window opens. The owner presses a
//! new key or types it. Each other GNOME key binding with that key shows
//! with a Remove button. "Set" writes the key only when no conflict is
//! left.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;
use petlauncher_config::Severity;
use petlauncher_gnome::{CustomShortcuts, GnomeDesktop, SHORTCUTS_PAGE, SHORTCUT_NAME};

use super::hotkey::{capture, Capture, HotkeyState};
use super::widgets::{set_text, MessageRow};
use super::Context;
use crate::global_key::key_label as label;
use crate::log;

pub struct KeyPage {
    pub page: adw::PreferencesPage,
    state: Rc<RefCell<HotkeyState>>,
    widgets: Rc<KeyWidgets>,
}

struct KeyWidgets {
    current: adw::ActionRow,
    clear: gtk::Button,
    candidate: adw::EntryRow,
    press: gtk::ToggleButton,
    status: MessageRow,
    set: gtk::Button,
    conflicts: adw::PreferencesGroup,
    conflict_rows: RefCell<Vec<adw::ActionRow>>,
    unavailable: MessageRow,
    /// True while the window sets the entry, so `changed` does nothing.
    showing: Cell<bool>,
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
        let current = adw::ActionRow::builder()
            .title("Key now")
            .use_markup(false)
            .build();
        let clear = gtk::Button::builder()
            .label("Remove Key")
            .tooltip_text("petlauncher then has no global key")
            .valign(gtk::Align::Center)
            .build();
        current.add_suffix(&clear);
        let candidate = adw::EntryRow::builder().title("New key").build();
        let press = gtk::ToggleButton::builder()
            .label("Press a Key")
            .valign(gtk::Align::Center)
            .build();
        candidate.add_suffix(&press);
        let status = MessageRow::new();
        let set = gtk::Button::builder()
            .label("Set")
            .valign(gtk::Align::Center)
            .build();
        set.add_css_class("suggested-action");
        status.row.add_suffix(&set);
        group.add(&unavailable.row);
        group.add(&current);
        group.add(&candidate);
        group.add(&status.row);
        page.add(&group);

        let conflicts = adw::PreferencesGroup::builder()
            .title("Key bindings with the same key")
            .visible(false)
            .build();
        page.add(&conflicts);

        let help = adw::PreferencesGroup::builder()
            .description(glib::markup_escape_text(
                "Press a Key: press the new key with its modifiers, or Escape to stop. \
                 GNOME Shell keeps some keys for itself, and they do not reach this window: \
                 type such a key, for example <Super>space or <Control><Alt>t.",
            ))
            .build();
        page.add(&help);

        let this = Self {
            page,
            state: Rc::default(),
            widgets: Rc::new(KeyWidgets {
                current,
                clear,
                candidate,
                press,
                status,
                set,
                conflicts,
                conflict_rows: RefCell::default(),
                unavailable,
                showing: Cell::new(false),
            }),
        };
        this.connect(context);
        this
    }

    fn connect(&self, context: &Rc<Context>) {
        let (state, widgets) = (&self.state, &self.widgets);
        let on_change = handler(context, state, widgets);
        widgets.candidate.connect_changed(move |entry| {
            on_change(
                &mut |state, shortcuts| state.set_candidate(shortcuts, &entry.text()),
                false,
            );
        });

        let on_set = handler(context, state, widgets);
        let weak = Rc::downgrade(context);
        widgets.set.connect_clicked(move |_| {
            let Some(context) = weak.upgrade() else {
                return;
            };
            on_set(
                &mut |state, shortcuts| match state.set(shortcuts) {
                    Ok(true) => {
                        let key = state.current.clone();
                        log::info(format_args!("settings window: global key {key:?} set"));
                        report(&context, key.as_deref());
                    }
                    Ok(false) => {}
                    Err(error) => context.toast(&error),
                },
                false,
            );
        });

        let on_clear = handler(context, state, widgets);
        let weak = Rc::downgrade(context);
        widgets.clear.connect_clicked(move |_| {
            let Some(context) = weak.upgrade() else {
                return;
            };
            on_clear(
                &mut |state, shortcuts| match state.clear(shortcuts) {
                    Ok(()) => {
                        log::info("settings window: global key removed");
                        report(&context, None);
                    }
                    Err(error) => context.toast(&error),
                },
                true,
            );
        });

        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak_widgets = Rc::downgrade(widgets);
        controller.connect_key_pressed(move |_, keyval, _, modifiers| {
            let Some(widgets) = weak_widgets.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if !widgets.press.is_active() {
                return glib::Propagation::Proceed;
            }
            match capture(keyval, modifiers) {
                Capture::Wait => {}
                Capture::Cancel => widgets.press.set_active(false),
                Capture::Key(text) => {
                    log::debug(format_args!("settings window: pressed {text}"));
                    widgets.press.set_active(false);
                    widgets.candidate.set_text(&text);
                }
            }
            glib::Propagation::Stop
        });
        context.window.add_controller(controller);
        widgets.press.connect_active_notify(|button| {
            button.set_label(if button.is_active() {
                "Press the Key Now"
            } else {
                "Press a Key"
            });
        });
    }

    /// Reads the key of GNOME. The window calls it each time it opens.
    pub fn load(&self, context: &Context) {
        let (current, error) = match shortcuts(context) {
            Ok(shortcuts) => {
                let state = HotkeyState::load(shortcuts, &context.backend.settings().hotkey);
                log::debug(format_args!(
                    "settings window: global key {:?}, {} conflicts",
                    state.current,
                    state.conflicts.len()
                ));
                (state, None)
            }
            Err(error) => (HotkeyState::default(), Some(error)),
        };
        self.state.replace(current);
        self.widgets
            .unavailable
            .set(error.as_deref().map(|error| (Severity::Error, error)));
        self.widgets.press.set_active(false);
    }

    pub fn reload(&self, context: &Rc<Context>) {
        let available = shortcuts(context).is_ok();
        for widget in [
            self.widgets.current.upcast_ref::<gtk::Widget>(),
            self.widgets.candidate.upcast_ref(),
            self.widgets.status.row.upcast_ref(),
        ] {
            widget.set_sensitive(available);
        }
        show(context, &self.state, &self.widgets);
    }
}

type Change<'a> = dyn FnMut(&mut HotkeyState, &CustomShortcuts) + 'a;

/// A handler that changes the state with the GNOME key bindings and then
/// shows it. With `entry`, the entry shows the new candidate.
fn handler(
    context: &Rc<Context>,
    state: &Rc<RefCell<HotkeyState>>,
    widgets: &Rc<KeyWidgets>,
) -> impl Fn(&mut Change, bool) + 'static {
    let context = Rc::downgrade(context);
    let state = Rc::downgrade(state);
    let widgets = Rc::downgrade(widgets);
    move |change, entry| {
        let (Some(context), Some(state), Some(widgets)) =
            (context.upgrade(), state.upgrade(), widgets.upgrade())
        else {
            return;
        };
        if widgets.showing.get() {
            return;
        }
        let Ok(shortcuts) = shortcuts(&context) else {
            return;
        };
        change(&mut state.borrow_mut(), shortcuts);
        if entry {
            widgets.showing.set(true);
            set_text(&widgets.candidate, &state.borrow().candidate);
            widgets.showing.set(false);
        }
        show(&context, &state, &widgets);
    }
}

fn shortcuts(context: &Context) -> Result<&CustomShortcuts, String> {
    context
        .backend
        .desktop()
        .and_then(GnomeDesktop::shortcuts)
        .map_err(|error| format!("The GNOME key bindings are not available: {error}"))
}

/// After a write of the key: the program writes the setting and ends the
/// first installation.
fn report(context: &Context, key: Option<&str>) {
    let result = match context.host() {
        Some(host) => host.global_key_changed(key),
        None => Ok(()),
    };
    match result {
        Ok(()) => context.toast(&match key {
            Some(key) => format!("petlauncher has the global key {}", label(key)),
            None => "petlauncher has no global key".to_owned(),
        }),
        Err(error) => context.toast(&format!("GNOME has the change, but {error}")),
    }
}

fn show(context: &Rc<Context>, state: &Rc<RefCell<HotkeyState>>, widgets: &Rc<KeyWidgets>) {
    let current = state.borrow();
    widgets.current.set_subtitle(&match &current.current {
        Some(key) => format!("{}    {key}", label(key)),
        None => "No key".to_owned(),
    });
    widgets.clear.set_sensitive(current.can_clear());
    widgets.showing.set(true);
    if widgets.candidate.text().trim() != current.candidate {
        widgets.candidate.set_text(&current.candidate);
    }
    widgets.showing.set(false);
    let severity = if current.error.is_some() || !current.conflicts.is_empty() {
        Severity::Error
    } else {
        Severity::Warning
    };
    let status = current.status(label);
    if severity == Severity::Error {
        widgets.status.set(Some((severity, &status)));
    } else {
        widgets.status.note("dialog-information-symbolic", &status);
    }
    widgets.set.set_visible(current.can_set());

    for row in widgets.conflict_rows.take() {
        widgets.conflicts.remove(&row);
    }
    widgets.conflicts.set_visible(!current.conflicts.is_empty());
    widgets
        .conflicts
        .set_description(Some(&glib::markup_escape_text(&format!(
            "These key bindings of GNOME have {}. Remove the key from each of them to use it for \
         petlauncher. Remove changes only the key of the binding, and the binding stays.",
            label(&current.candidate)
        ))));
    let mut rows = Vec::new();
    for (index, conflict) in current.conflicts.iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(conflict.name.as_str())
            .subtitle(conflict.place())
            .use_markup(false)
            .build();
        let remove = gtk::Button::builder()
            .label("Remove")
            .tooltip_text(format!(
                "Remove {} from this key binding",
                conflict.accelerator
            ))
            .valign(gtk::Align::Center)
            .build();
        remove.add_css_class("destructive-action");
        let on_remove = handler(context, state, widgets);
        let weak = Rc::downgrade(context);
        remove.connect_clicked(move |_| {
            let Some(context) = weak.upgrade() else {
                return;
            };
            on_remove(
                &mut |state, shortcuts| {
                    if let Err(error) = state.remove_conflict(shortcuts, index) {
                        context.toast(&error);
                    } else {
                        log::info(format_args!(
                            "settings window: removed the key of conflict {index}, {} left",
                            state.conflicts.len()
                        ));
                    }
                },
                false,
            );
        });
        row.add_suffix(&remove);
        widgets.conflicts.add(&row);
        rows.push(row);
    }
    widgets.conflict_rows.replace(rows);
}
