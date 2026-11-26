//! The launcher window: the query line and the result list. The rules are
//! in `model`, `selection`, `keys` and `visibility`. This file connects them
//! to the widgets.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::Duration;

use adw::prelude::*;
use gtk::{gdk, glib};
use petlauncher_config::Settings;
use petlauncher_core::{CloseReason, Key, Outcome, Results};

use crate::backend::Backend;
use crate::dictionary_window;
use crate::keys::{self, Intent};
use crate::log;
use crate::model::{self, Arrival, Effect, LauncherModel, Press};
use crate::notices::{NoticeKind, Notices};
use crate::rows;
use crate::visibility::{Toggle, Visibility};

/// The title that the GNOME Shell extension looks for (ADR 0005). No other
/// window can have it.
pub const TITLE: &str = "petlauncher";

/// The height of the launcher with no rows, in logical pixels. The GNOME
/// Shell extension puts the top of the launcher where a window of this
/// height is at the center of the monitor (`COLLAPSED_HEIGHT` in
/// `gnome/extension/petlauncher@repo.pet/launcher.js`). Thus the query line
/// has the same place for each height of the window.
pub const COLLAPSED_HEIGHT: i32 = 64;

/// The number of rows that show before the list scrolls.
const VISIBLE_ROWS: i32 = 8;

/// The time from the hide to the stop when "Run in background" is off.
const STOP_DELAY: Duration = Duration::from_secs(1);

pub struct LauncherWindow {
    window: adw::ApplicationWindow,
    content: gtk::Box,
    entry: gtk::Entry,
    notice_box: gtk::Box,
    separator: gtk::Separator,
    scroller: gtk::ScrolledWindow,
    list: gtk::ListBox,
    backend: Rc<Backend>,
    model: RefCell<LauncherModel>,
    visibility: RefCell<Visibility>,
    /// The last FOCUSED state of the toplevel surface.
    focused: Cell<bool>,
    /// The show had a kept query: select all its text at the first focus.
    select_at_focus: Cell<bool>,
    notices: RefCell<Notices>,
    /// Runs at the start of each show, before the window is visible.
    before_show: RefCell<Option<Box<dyn Fn()>>>,
}

impl LauncherWindow {
    pub fn new(
        app: &adw::Application,
        backend: Rc<Backend>,
        results: async_channel::Receiver<Results>,
    ) -> Rc<Self> {
        let entry = gtk::Entry::builder()
            .placeholder_text("Search")
            .primary_icon_name("system-search-symbolic")
            .has_frame(false)
            .hexpand(true)
            .valign(gtk::Align::Center)
            .css_classes(["query"])
            .build();
        let query_bar = gtk::Box::builder()
            .height_request(COLLAPSED_HEIGHT)
            .css_classes(["query-bar"])
            .build();
        query_bar.append(&entry);
        let list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .activate_on_single_click(true)
            .can_focus(false)
            .css_classes(["results"])
            .build();
        let scroller = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .child(&list)
            .visible(false)
            .css_classes(["results-scroller"])
            .build();
        let notice_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .visible(false)
            .css_classes(["notices"])
            .build();
        let separator = gtk::Separator::builder().visible(false).build();
        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        content.append(&query_bar);
        content.append(&notice_box);
        content.append(&separator);
        content.append(&scroller);
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(TITLE)
            .resizable(false)
            .content(&content)
            .build();
        window.add_css_class("petlauncher");

        let this = Rc::new(Self {
            window,
            content,
            entry,
            notice_box,
            separator,
            scroller,
            list,
            backend,
            model: RefCell::new(LauncherModel::default()),
            visibility: RefCell::new(Visibility::default()),
            focused: Cell::new(false),
            select_at_focus: Cell::new(false),
            notices: RefCell::new(Notices::default()),
            before_show: RefCell::new(None),
        });
        this.set_width(this.backend.settings().window_width);
        this.connect_signals();
        this.connect_focus();
        this.receive_results(results);
        // The renderer of the window starts at the realize. Here it starts
        // with the program, not at the first show: in a test the first show
        // took 312 ms without this and 125 ms with it.
        WidgetExt::realize(&this.window);
        this
    }

    fn connect_signals(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.entry
            .connect_changed(with(&weak, |this| this.text_changed()));

        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak_keys = weak.clone();
        keys.connect_key_pressed(move |_, keyval, _, state| {
            match (weak_keys.upgrade(), keys::intent(keyval, state)) {
                (Some(this), Some(intent)) => {
                    this.key(intent);
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.window.add_controller(keys);

        let weak_rows = weak.clone();
        self.list.connect_row_activated(move |_, row| {
            if let (Some(this), Ok(index)) = (weak_rows.upgrade(), usize::try_from(row.index())) {
                this.model.borrow_mut().select(index);
                this.press(Key::Enter);
            }
        });

        self.window.connect_map(|_| log::debug("mapped"));
        let weak_close = weak;
        self.window.connect_close_request(move |_| {
            if let Some(this) = weak_close.upgrade() {
                this.hide(CloseReason::Escape);
            }
            glib::Propagation::Stop
        });
    }

    /// Follows the FOCUSED state of the toplevel surface. GTK 4.14 keeps the
    /// property `is-active` of the window true after a hide, so it does not
    /// tell the focus of the next show. The surface state does: the first
    /// configure of each show has no FOCUSED state, and GNOME Shell then
    /// activates the window.
    fn connect_focus(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.window.connect_realize(move |window| {
            let Some(toplevel) = window.surface().and_downcast::<gdk::Toplevel>() else {
                return;
            };
            let weak = weak.clone();
            toplevel.connect_state_notify(move |toplevel| {
                if let Some(this) = weak.upgrade() {
                    this.focus_changed(toplevel.state().contains(gdk::ToplevelState::FOCUSED));
                }
            });
        });
    }

    fn receive_results(self: &Rc<Self>, results: async_channel::Receiver<Results>) {
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            while let Ok(found) = results.recv().await {
                match weak.upgrade() {
                    Some(this) => this.results_arrived(found),
                    None => break,
                }
            }
        });
    }

    pub fn apply_settings(&self, settings: &Settings) {
        self.set_width(settings.window_width);
    }

    /// Runs `hook` at the start of each show, before the window is visible.
    pub fn set_before_show(&self, hook: impl Fn() + 'static) {
        self.before_show.replace(Some(Box::new(hook)));
    }

    /// R4: the notice that the global key is not set, or `None`.
    pub fn set_global_key_notice(&self, text: Option<String>) {
        self.notices.borrow_mut().set_global_key(text);
        self.show_notices();
    }

    /// R18: the error of a settings file, or `None`.
    pub fn set_config_notice(&self, text: Option<String>) {
        self.notices.borrow_mut().set_config(text);
        self.show_notices();
    }

    /// Searches the query again, for example after the file index or the
    /// applications changed. The selected row stays when it is still there.
    pub fn refresh(&self) {
        let text = self.model.borrow().text().to_owned();
        self.backend.search(&text);
    }

    /// The action `show`: shows the launcher. When it shows already, the
    /// query and the rows do not change, and the window only comes to the
    /// front.
    pub fn show(&self) {
        if !self.visibility.borrow_mut().show() {
            self.window.present();
            return;
        }
        if let Some(hook) = self.before_show.borrow().as_ref() {
            hook();
        }
        self.notices.borrow_mut().clear_action();
        self.show_notices();
        self.set_width(self.backend.settings().window_width);
        let kept = self.backend.window_opened();
        match &kept {
            Some(query) => self.restore(query),
            None => self.clear(),
        }
        // A selection before the focus does not stay: GTK claims the
        // primary selection for it, and on Wayland that claim fails while
        // the window has no keyboard focus. Then GTK removes the selection.
        self.select_at_focus.set(kept.is_some());
        self.entry.grab_focus();
        log::debug(format_args!("show, kept query {kept:?}"));
        self.window.present();
    }

    /// `petlauncher toggle`: the global key.
    pub fn toggle(&self) {
        let toggle = self.visibility.borrow().toggle();
        match toggle {
            Toggle::Show => self.show(),
            Toggle::Hide(reason) => self.hide(reason),
        }
    }

    pub fn hide(&self, reason: CloseReason) {
        if !self.visibility.borrow_mut().hide() {
            return;
        }
        // `executed` recorded the close after a key. A second record would
        // decide the kept query again (R9 step 5).
        if !matches!(reason, CloseReason::Activated { .. }) {
            let text = self.model.borrow().text().to_owned();
            self.backend.window_closed(&text, reason);
        }
        log::debug(format_args!("hide, {reason:?}"));
        self.window.set_visible(false);
        if !self.backend.settings().run_in_background {
            self.stop_later();
        }
    }

    /// "Run in background" is off: the process stops when the window is
    /// still hidden after `STOP_DELAY`. On Wayland the text of a Copy
    /// belongs to this process. GNOME Shell keeps a copy of the clipboard
    /// when its owner stops, but only after it read the text: a test showed
    /// the previous clipboard text after a stop at once.
    fn stop_later(&self) {
        let window = self.window.clone();
        glib::timeout_add_local_once(STOP_DELAY, move || {
            if window.is_visible() {
                return;
            }
            if let Some(app) = window.application() {
                log::info("\"Run in background\" is off: stop");
                app.quit();
            }
        });
    }

    fn set_width(&self, width: u32) {
        let width = i32::try_from(width).unwrap_or(i32::MAX);
        self.content.set_size_request(width, -1);
        self.window.set_default_size(width, -1);
    }

    /// A show with no kept query: the window shows at its collapsed height.
    fn clear(&self) {
        self.model.borrow_mut().clear();
        self.show_rows();
        self.entry.set_text("");
    }

    /// A show with the kept query of R9: the window shows with the rows of
    /// the query, and the search runs again.
    fn restore(&self, query: &str) {
        self.model.borrow_mut().restore(query);
        self.show_rows();
        if self.entry.text() == query {
            self.backend.search(query);
        } else {
            self.entry.set_text(query);
        }
    }

    fn text_changed(&self) {
        let text = self.entry.text();
        log::debug(format_args!("search {text:?}"));
        if self.notices.borrow_mut().clear_action() {
            self.show_notices();
        }
        self.model.borrow_mut().set_text(&text);
        self.backend.search(&text);
    }

    fn results_arrived(&self, results: Results) {
        let same_query = self.model.borrow().results().query == results.query;
        let arrival = self.model.borrow_mut().results_arrived(results);
        if arrival == Arrival::Old {
            return;
        }
        let scroll = self.scroller.vadjustment().value();
        self.show_rows();
        if same_query {
            self.scroller.vadjustment().set_value(scroll);
        }
        log::debug(self.rows_line());
        if let Arrival::ShowAndActivate(key) = arrival {
            self.activate(key);
        }
    }

    /// The rows that show, for the debug log of the tests.
    fn rows_line(&self) -> String {
        let model = self.model.borrow();
        let results = model.results();
        let titles: Vec<&str> = results
            .rows
            .iter()
            .take(5)
            .map(|row| row.title.as_str())
            .collect();
        format!(
            "rows for {:?}: {}, selected {:?}: {}",
            results.query,
            results.rows.len(),
            model.selected(),
            titles.join(" | ")
        )
    }

    fn focus_changed(&self, focused: bool) {
        if self.focused.replace(focused) == focused {
            return;
        }
        log::debug(format_args!("focused {focused}"));
        if focused {
            let first = self.visibility.borrow_mut().activated();
            if first && self.select_at_focus.take() {
                // After the events that GDK has now, so that the keyboard
                // focus is also there.
                let entry = self.entry.clone();
                glib::idle_add_local_once(move || entry.select_region(0, -1));
            }
            return;
        }
        let close_on_focus_loss = self.backend.settings().close_on_focus_loss;
        let reason = self.visibility.borrow().deactivated(close_on_focus_loss);
        if let Some(reason) = reason {
            self.hide(reason);
        }
    }

    fn key(&self, intent: Intent) {
        match intent {
            Intent::Activate(key) => self.press(key),
            Intent::Up => {
                self.model.borrow_mut().up();
                self.show_selection();
            }
            Intent::Down => {
                self.model.borrow_mut().down();
                self.show_selection();
            }
            Intent::Hide => self.hide(CloseReason::Escape),
            Intent::Complete => {
                let completion = self.model.borrow().completion().map(str::to_owned);
                if let Some(text) = completion {
                    self.set_query(&text);
                }
            }
        }
    }

    fn press(&self, key: Key) {
        let press = self.model.borrow_mut().press(key);
        if press == Press::Now {
            self.activate(key);
        }
    }

    fn activate(&self, key: Key) {
        let outcome = {
            let model = self.model.borrow();
            self.backend
                .activate(model.results(), model.selected(), key)
        };
        self.execute(&outcome);
    }

    fn execute(&self, outcome: &Outcome) {
        log::debug(format_args!("outcome {outcome:?}"));
        match model::effect(&outcome.action) {
            Effect::Nothing => {}
            Effect::SetQuery(text) => self.set_query(text),
            Effect::Copy(text) => self.window.clipboard().set_text(text),
            Effect::Dictionary(word) => dictionary_window::open(word),
            Effect::Desktop => {
                if let Err(error) = self.backend.execute(&outcome.action) {
                    // The window stays open and shows the error.
                    log::info(format_args!("action error: {error}"));
                    self.notices
                        .borrow_mut()
                        .set_action(Some(error.to_string()));
                    self.show_notices();
                    return;
                }
                log::debug(format_args!("executed {:?}", outcome.action));
            }
        }
        let query = self.model.borrow().results().query.clone();
        self.backend.executed(&query, outcome);
        if outcome.close {
            self.hide(CloseReason::Activated {
                keep_query: outcome.keep_query,
            });
        }
    }

    fn show_notices(&self) {
        while let Some(child) = self.notice_box.first_child() {
            self.notice_box.remove(&child);
        }
        let notices = self.notices.borrow();
        let lines = notices.lines();
        for (kind, text) in &lines {
            let (icon, class) = match kind {
                NoticeKind::Error => ("dialog-error-symbolic", "error"),
                NoticeKind::Warning => ("dialog-warning-symbolic", "warning"),
            };
            let line = gtk::Box::builder()
                .spacing(10)
                .css_classes(["notice"])
                .build();
            line.append(
                &gtk::Image::builder()
                    .icon_name(icon)
                    .valign(gtk::Align::Start)
                    .css_classes([class])
                    .build(),
            );
            line.append(
                &gtk::Label::builder()
                    .label(*text)
                    .wrap(true)
                    .wrap_mode(gtk::pango::WrapMode::WordChar)
                    .xalign(0.0)
                    .hexpand(true)
                    .max_width_chars(1)
                    .selectable(false)
                    .build(),
            );
            self.notice_box.append(&line);
        }
        self.notice_box.set_visible(!lines.is_empty());
    }

    fn set_query(&self, text: &str) {
        self.entry.set_text(text);
        self.entry.set_position(-1);
    }

    fn show_rows(&self) {
        self.list.remove_all();
        let has_rows = {
            let model = self.model.borrow();
            for row in &model.results().rows {
                self.list.append(&rows::row_widget(row));
            }
            !model.results().rows.is_empty()
        };
        self.separator.set_visible(has_rows);
        self.scroller.set_visible(has_rows);
        if let Some(first) = self.list.row_at_index(0) {
            let (_, row_height, _, _) = first.measure(gtk::Orientation::Vertical, -1);
            self.scroller
                .set_max_content_height(row_height * VISIBLE_ROWS);
        }
        self.scroller.vadjustment().set_value(0.0);
        self.show_selection();
    }

    fn show_selection(&self) {
        let selected = self.model.borrow().selected();
        let row = selected
            .and_then(|index| i32::try_from(index).ok())
            .and_then(|index| self.list.row_at_index(index));
        match row {
            Some(row) => {
                self.list.select_row(Some(&row));
                self.scroll_to(&row);
            }
            None => self.list.unselect_all(),
        }
    }

    /// Scrolls the list so that the row is fully visible.
    fn scroll_to(&self, row: &gtk::ListBoxRow) {
        let Some(bounds) = row.compute_bounds(&self.list) else {
            return;
        };
        let adjustment = self.scroller.vadjustment();
        let top = f64::from(bounds.y());
        let bottom = top + f64::from(bounds.height());
        if top < adjustment.value() {
            adjustment.set_value(top);
        } else if bottom > adjustment.value() + adjustment.page_size() {
            adjustment.set_value(bottom - adjustment.page_size());
        }
    }
}

/// A signal handler that calls `f` while the window exists.
fn with<W: ?Sized>(
    weak: &Weak<LauncherWindow>,
    f: impl Fn(&LauncherWindow) + 'static,
) -> impl Fn(&W) + 'static {
    let weak = weak.clone();
    move |_| {
        if let Some(this) = weak.upgrade() {
            f(&this);
        }
    }
}
