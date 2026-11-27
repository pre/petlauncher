//! The dialog that sets the global key (R4, ADR 0008). It records the next
//! key combination: Escape closes it, and BackSpace offers no key. Each other
//! GNOME key binding with the pressed key shows with a Remove button. Set
//! writes the key only when no conflict is left. The logic is in `hotkey`.
//!
//! GNOME Shell takes its own keys, for example Super+Space, before a window
//! gets them. While the dialog shows, it asks GNOME Shell to pause its
//! shortcuts, and it shows the answer. GNOME Shell can ask the owner first.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, glib};

use super::hotkey::{capture, Capture, HotkeyState};
use super::key_page::shortcuts;
use super::Context;
use crate::global_key::key_label;
use crate::log;

/// Opens the dialog with the key of GNOME, or with the setting `hotkey`
/// when GNOME has no key for petlauncher.
pub fn present(context: &Rc<Context>) {
    let Ok(shortcuts) = shortcuts(context) else {
        return;
    };
    let state = HotkeyState::load(shortcuts, &context.backend.settings().hotkey);
    let dialog = KeyDialog::new(context, state);
    dialog.dialog.present(Some(&context.window));
    // The dialog keeps itself until its response, which comes after its
    // signal "closed" and after the handler of Set.
    let keep = RefCell::new(Some(Rc::clone(&dialog)));
    dialog.dialog.connect_response(None, move |_, _| {
        keep.take();
    });
}

struct KeyDialog {
    dialog: adw::AlertDialog,
    key: gtk::Label,
    inhibit: gtk::Label,
    status: gtk::Label,
    conflicts: gtk::ListBox,
    state: RefCell<HotkeyState>,
    context: Weak<Context>,
}

impl KeyDialog {
    fn new(context: &Rc<Context>, state: HotkeyState) -> Rc<Self> {
        let key = gtk::Label::builder().css_classes(["title-2"]).build();
        let inhibit = label(&["dim-label"]);
        let status = label(&[]);
        let conflicts = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .build();
        for widget in [
            key.upcast_ref::<gtk::Widget>(),
            inhibit.upcast_ref(),
            status.upcast_ref(),
            conflicts.upcast_ref(),
        ] {
            content.append(widget);
        }
        let dialog = adw::AlertDialog::builder()
            .heading("Set the Global Key")
            .body(
                "Press the new key combination. Press Escape to cancel, or BackSpace to \
                 remove the global key.",
            )
            .extra_child(&content)
            .close_response("cancel")
            .build();
        dialog.add_responses(&[("cancel", "Cancel"), ("set", "Set")]);
        dialog.set_response_appearance("set", adw::ResponseAppearance::Suggested);

        let this = Rc::new(Self {
            dialog,
            key,
            inhibit,
            status,
            conflicts,
            state: RefCell::new(state),
            context: Rc::downgrade(context),
        });
        this.connect();
        this.pause_shell_shortcuts();
        show_inhibited(&this.inhibit, false);
        this.show();
        this
    }

    fn connect(self: &Rc<Self>) {
        // The capture phase gets the keys before the buttons of the dialog.
        let controller = gtk::EventControllerKey::new();
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        controller.connect_key_pressed(move |_, keyval, _, modifiers| {
            if let Some(this) = weak.upgrade() {
                match capture(keyval, modifiers) {
                    Capture::Wait => {}
                    Capture::Cancel => {
                        this.dialog.close();
                    }
                    Capture::Clear => this.offer(""),
                    Capture::Key(text) => this.offer(&text),
                }
            }
            glib::Propagation::Stop
        });
        self.dialog.add_controller(controller);

        let weak = Rc::downgrade(self);
        self.dialog.connect_response(Some("set"), move |_, _| {
            if let Some(this) = weak.upgrade() {
                this.set();
            }
        });
    }

    /// The pause is on the surface of the settings window, because the
    /// dialog shows inside that window. The handlers of the map and the
    /// unmap keep the paused surface themselves: the last reference to
    /// `KeyDialog` goes at the response, before the unmap. Without the
    /// stop at the unmap, GNOME Shell ignores its keys, also the global key
    /// of petlauncher, while the settings window has the focus.
    fn pause_shell_shortcuts(&self) {
        let paused: Rc<RefCell<Option<(gdk::Toplevel, glib::SignalHandlerId)>>> = Rc::default();
        let at_map = Rc::clone(&paused);
        let label = self.inhibit.clone();
        self.dialog.connect_map(move |dialog| {
            let Some(toplevel) = dialog
                .native()
                .and_then(|native| native.surface())
                .and_downcast::<gdk::Toplevel>()
            else {
                return;
            };
            toplevel.inhibit_system_shortcuts(None::<&gdk::Event>);
            show_inhibited(&label, toplevel.is_shortcuts_inhibited());
            let label = label.clone();
            let handler = toplevel.connect_shortcuts_inhibited_notify(move |toplevel| {
                show_inhibited(&label, toplevel.is_shortcuts_inhibited());
            });
            at_map.replace(Some((toplevel, handler)));
        });
        self.dialog.connect_unmap(move |_| {
            if let Some((toplevel, handler)) = paused.take() {
                toplevel.restore_system_shortcuts();
                toplevel.disconnect(handler);
                log::debug("settings window: shortcuts of GNOME Shell restored");
            }
        });
    }

    /// A key press: the new candidate.
    fn offer(self: &Rc<Self>, key: &str) {
        let Some(context) = self.context.upgrade() else {
            return;
        };
        let Ok(shortcuts) = shortcuts(&context) else {
            return;
        };
        log::debug(format_args!("settings window: pressed {key:?}"));
        self.state.borrow_mut().set_candidate(shortcuts, key);
        self.show();
    }

    fn show(self: &Rc<Self>) {
        let state = self.state.borrow();
        self.key.set_label(&if state.candidate.is_empty() {
            "No key".to_owned()
        } else {
            key_label(&state.candidate)
        });
        self.status.set_label(&state.status(key_label));
        let problem = state.error.is_some() || !state.conflicts.is_empty();
        self.status
            .set_css_classes(if problem { &["error"] } else { &["dim-label"] });
        self.conflicts.remove_all();
        for (index, conflict) in state.conflicts.iter().enumerate() {
            self.conflicts.append(&self.conflict_row(index, conflict));
        }
        self.conflicts.set_visible(!state.conflicts.is_empty());
        self.dialog.set_response_enabled("set", state.can_set());
    }

    /// A row with the name of a different key binding, its section in GNOME
    /// Settings, and a button that removes the key from that binding only.
    fn conflict_row(
        self: &Rc<Self>,
        index: usize,
        conflict: &petlauncher_gnome::Conflict,
    ) -> adw::ActionRow {
        let row = adw::ActionRow::builder()
            .title(conflict.name.as_str())
            .subtitle(conflict.section.clone().unwrap_or_else(|| conflict.place()))
            .use_markup(false)
            .build();
        let remove = gtk::Button::builder()
            .label("Remove")
            .tooltip_text(format!(
                "Remove {} from this key binding",
                conflict.accelerator
            ))
            .valign(gtk::Align::Center)
            .css_classes(["destructive-action"])
            .build();
        let context = self.context.clone();
        let weak = Rc::downgrade(self);
        remove.connect_clicked(move |_| {
            let (Some(this), Some(context)) = (weak.upgrade(), context.upgrade()) else {
                return;
            };
            let Ok(shortcuts) = shortcuts(&context) else {
                return;
            };
            let result = this.state.borrow_mut().remove_conflict(shortcuts, index);
            match result {
                Ok(()) => log::info(format_args!(
                    "settings window: removed the key of conflict {index}, {} left",
                    this.state.borrow().conflicts.len()
                )),
                Err(error) => context.toast(&error),
            }
            this.show();
        });
        row.add_suffix(&remove);
        row
    }

    /// Set: writes the key in GNOME, then the program writes the setting and
    /// ends the first installation.
    fn set(&self) {
        let Some(context) = self.context.upgrade() else {
            return;
        };
        let Ok(shortcuts) = shortcuts(&context) else {
            return;
        };
        let result = self.state.borrow_mut().set(shortcuts);
        match result {
            Ok(true) => {
                let key = self.state.borrow().current.clone();
                log::info(format_args!("settings window: global key {key:?} set"));
                report(&context, key.as_deref());
            }
            Ok(false) => {
                context.toast("The key was not set, because a different key binding has it now.")
            }
            Err(error) => context.toast(&error),
        }
        context.reload_later();
    }
}

fn show_inhibited(label: &gtk::Label, inhibited: bool) {
    log::debug(format_args!(
        "settings window: shortcuts of GNOME Shell paused {inhibited}"
    ));
    label.set_label(if inhibited {
        "GNOME Shell pauses its shortcuts, so all keys come to this dialog."
    } else {
        "GNOME Shell did not pause its shortcuts. When a key press does not show here, \
         GNOME Shell uses that key."
    });
}

fn label(classes: &[&str]) -> gtk::Label {
    gtk::Label::builder()
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(classes)
        .build()
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
            Some(key) => format!("petlauncher has the global key {}", key_label(key)),
            None => "petlauncher has no global key".to_owned(),
        }),
        Err(error) => context.toast(&format!("GNOME has the change, but {error}")),
    }
}
