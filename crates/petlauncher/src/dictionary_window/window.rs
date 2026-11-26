//! The dictionary window: the header bar with the history buttons and the
//! search field, and a stack with the pages. The rules are in `history`,
//! `page`, `keys` and `markup`. This file connects them to the widgets.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use adw::prelude::*;
use gtk::glib;

use super::history::History;
use super::keys::{self, Escape, Intent};
use super::page::{normalize, search_target, Page, Problem};
use super::source::{DictionarySource, SourceError};
use super::view::{self, OpenWord};
use super::TITLE;
use crate::log;
use crate::selection::Selection;

/// The number of words that the search field suggests.
const SUGGESTIONS: usize = 12;

/// Up and Down scroll the page by this number of pixels.
const SCROLL_STEP: f64 = 48.0;

/// With "Run in background" off, the process stops this time after the
/// last window hides: the same rule and the same reason (the clipboard) as
/// for the launcher in `window.rs`.
const STOP_DELAY: Duration = Duration::from_secs(1);

/// The names of the pages in the stack.
const WORD: &str = "word";
const SUGGESTIONS_PAGE: &str = "suggestions";
const NOT_FOUND: &str = "not-found";
const UNAVAILABLE: &str = "unavailable";

pub struct DictionaryWindow {
    /// For the closures of the links, which the window makes for each page.
    me: Weak<Self>,
    window: adw::ApplicationWindow,
    search: gtk::SearchEntry,
    back: gtk::Button,
    forward: gtk::Button,
    stack: gtk::Stack,
    word_scroller: gtk::ScrolledWindow,
    word_clamp: adw::Clamp,
    suggestion_list: gtk::ListBox,
    not_found: adw::StatusPage,
    unavailable: adw::StatusPage,
    problem: ProblemLabels,
    source: Rc<dyn DictionarySource>,
    run_in_background: Rc<dyn Fn() -> bool>,
    history: RefCell<History>,
    suggestions: RefCell<(Vec<String>, Selection)>,
    /// The name of the page of the current word, which shows again when the
    /// search field becomes empty.
    current_page: Cell<&'static str>,
    /// The scroll position to restore after the next layout of the page.
    pending_scroll: Cell<Option<f64>>,
}

/// The selectable text of the page for missing or bad data.
struct ProblemLabels {
    cause: gtk::Label,
    path: gtk::Label,
    command: gtk::Label,
}

impl DictionaryWindow {
    pub fn new(
        app: &adw::Application,
        source: Rc<dyn DictionarySource>,
        run_in_background: Rc<dyn Fn() -> bool>,
    ) -> Rc<Self> {
        let back = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Back (Alt+Left)")
            .sensitive(false)
            .build();
        let forward = gtk::Button::builder()
            .icon_name("go-next-symbolic")
            .tooltip_text("Forward (Alt+Right)")
            .sensitive(false)
            .build();
        let search = gtk::SearchEntry::builder()
            .placeholder_text("Search for a word")
            .hexpand(true)
            .build();
        let search_clamp = adw::Clamp::builder()
            .maximum_size(420)
            .child(&search)
            .build();
        let header = adw::HeaderBar::builder()
            .title_widget(&search_clamp)
            .build();
        header.pack_start(&back);
        header.pack_start(&forward);

        // The text of a meaning is about 70 characters wide at most.
        let word_clamp = adw::Clamp::builder()
            .maximum_size(600)
            .tightening_threshold(480)
            .build();
        let word_scroller = scroller(&word_clamp);
        let suggestion_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .activate_on_single_click(true)
            .css_classes(["boxed-list"])
            .valign(gtk::Align::Start)
            .build();
        let suggestion_clamp = adw::Clamp::builder()
            .maximum_size(620)
            .child(&suggestion_list)
            .css_classes(["dictionary-suggestions"])
            .build();
        let not_found = adw::StatusPage::builder()
            .icon_name("edit-find-symbolic")
            .build();
        let (unavailable, problem) = unavailable_page();

        let stack = gtk::Stack::builder().vexpand(true).build();
        stack.add_named(&word_scroller, Some(WORD));
        stack.add_named(&scroller(&suggestion_clamp), Some(SUGGESTIONS_PAGE));
        stack.add_named(&not_found, Some(NOT_FOUND));
        stack.add_named(&unavailable, Some(UNAVAILABLE));

        let view = adw::ToolbarView::builder().content(&stack).build();
        view.add_top_bar(&header);
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(TITLE)
            .default_width(720)
            .default_height(820)
            .hide_on_close(true)
            .content(&view)
            .build();
        window.add_css_class("dictionary-window");
        search.set_key_capture_widget(Some(&window));

        let this = Rc::new_cyclic(|me| Self {
            me: me.clone(),
            window,
            search,
            back,
            forward,
            stack,
            word_scroller,
            word_clamp,
            suggestion_list,
            not_found,
            unavailable,
            problem,
            source,
            run_in_background,
            history: RefCell::default(),
            suggestions: RefCell::default(),
            current_page: Cell::new(WORD),
            pending_scroll: Cell::new(None),
        });
        this.connect_signals();
        this
    }

    fn connect_signals(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.back
            .connect_clicked(with(&weak, |this| this.go(History::back)));
        self.forward
            .connect_clicked(with(&weak, |this| this.go(History::forward)));
        self.search
            .connect_changed(with(&weak, |this| this.search_changed()));
        self.search
            .connect_activate(with(&weak, |this| this.search_activated()));

        let weak_rows = weak.clone();
        self.suggestion_list.connect_row_activated(move |_, row| {
            let index = usize::try_from(row.index()).ok();
            if let (Some(this), Some(index)) = (weak_rows.upgrade(), index) {
                let word = this.suggestions.borrow().0.get(index).cloned();
                if let Some(word) = word {
                    this.show_word(&word);
                }
            }
        });

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

        // The back and forward buttons of a mouse.
        let buttons = gtk::GestureClick::builder().button(0).build();
        buttons.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak_buttons = weak.clone();
        buttons.connect_pressed(move |gesture, _, _, _| {
            let Some(this) = weak_buttons.upgrade() else {
                return;
            };
            match gesture.current_button() {
                8 => this.go(History::back),
                9 => this.go(History::forward),
                _ => {}
            }
        });
        self.window.add_controller(buttons);

        let weak_scroll = weak.clone();
        self.word_scroller
            .vadjustment()
            .connect_changed(move |adjustment| {
                let Some(scroll) = weak_scroll
                    .upgrade()
                    .and_then(|this| this.pending_scroll.take())
                else {
                    return;
                };
                // The viewport emits `changed` in its size allocation, and
                // a new value at that time does not move the page.
                let adjustment = adjustment.clone();
                glib::idle_add_local_once(move || {
                    adjustment.set_value(scroll);
                    log::debug(format_args!(
                        "dictionary window: scroll {:.0} of {:.0} restored",
                        adjustment.value(),
                        adjustment.upper()
                    ));
                });
            });

        let run_in_background = Rc::clone(&self.run_in_background);
        self.window.connect_close_request(move |window| {
            log::debug("dictionary window: hide");
            if !run_in_background() {
                stop_later(window);
            }
            glib::Propagation::Proceed
        });
    }

    /// Enter on a dictionary row of the launcher: shows the word in the
    /// window, over the word that it showed.
    pub fn open(&self, word: &str) {
        self.search.set_text("");
        self.show_word(word);
        // Before `present`: else GTK gives the first label the focus at the
        // map, and a selectable label selects all its text at the focus.
        self.search.grab_focus();
        self.window.present();
    }

    /// Makes the word the current word of the history and shows it.
    fn show_word(&self, word: &str) {
        let word = normalize(word);
        if word.is_empty() {
            return;
        }
        self.keep_scroll();
        self.history.borrow_mut().visit(&word);
        self.search.set_text("");
        self.show_current();
    }

    /// Back or forward.
    fn go(&self, step: fn(&mut History) -> bool) {
        self.keep_scroll();
        if step(&mut self.history.borrow_mut()) {
            self.search.set_text("");
            self.show_current();
        }
    }

    fn keep_scroll(&self) {
        if self.current_page.get() != WORD {
            return;
        }
        let scroll = self.word_scroller.vadjustment().value();
        log::debug(format_args!("dictionary window: scroll {scroll:.0} kept"));
        self.history.borrow_mut().set_scroll(scroll);
    }

    fn show_current(&self) {
        let Some(word) = self.history.borrow().current().map(str::to_owned) else {
            return;
        };
        let start = Instant::now();
        let page = Page::for_word(self.source.as_ref(), &word);
        let lookup = start.elapsed();
        self.show_page(&page);
        log::debug(format_args!(
            "dictionary window: page {word:?}, {} meanings, lookup {:.2} ms, widgets {:.1} ms",
            page.meanings(),
            lookup.as_secs_f64() * 1000.0,
            (start.elapsed() - lookup).as_secs_f64() * 1000.0
        ));
        let history = self.history.borrow();
        self.back.set_sensitive(history.can_go_back());
        self.forward.set_sensitive(history.can_go_forward());
    }

    fn show_page(&self, page: &Page) {
        // The new page removes the labels of the old page. GTK 4.14 writes
        // a critical message when it removes the label with the focus, for
        // example after a click on a link.
        if gtk::prelude::RootExt::focus(&self.window)
            .is_some_and(|focus| focus.is_ancestor(&self.stack))
        {
            self.search.grab_focus();
        }
        let name = match page {
            Page::Word { word, entries } => {
                let content = view::word_page(word, entries, &self.opener());
                self.word_clamp.set_child(Some(&content));
                let scroll = self.history.borrow().scroll();
                self.word_scroller.vadjustment().set_value(scroll);
                self.pending_scroll.set((scroll > 0.0).then_some(scroll));
                WORD
            }
            Page::NotFound { word, nearest } => {
                self.not_found
                    .set_title(&format!("“{word}” is not in the dictionary"));
                if nearest.is_empty() {
                    self.not_found.set_description(None);
                    self.not_found.set_child(gtk::Widget::NONE);
                } else {
                    self.not_found.set_description(Some("Similar words"));
                    self.not_found
                        .set_child(Some(&view::word_links(nearest, &self.opener())));
                }
                NOT_FOUND
            }
            Page::Unavailable(error) => {
                self.show_problem(error);
                UNAVAILABLE
            }
        };
        self.current_page.set(name);
        self.stack.set_visible_child_name(name);
    }

    fn show_problem(&self, error: &SourceError) {
        let problem = Problem::of(error);
        self.unavailable.set_title(problem.title);
        self.problem.cause.set_label(&problem.cause);
        self.problem.path.set_label(&problem.path);
        self.problem.command.set_label(problem.command);
    }

    /// The target of the links. The page changes after the signal of the
    /// link, because the change removes the label of the link.
    fn opener(&self) -> OpenWord {
        let weak = self.me.clone();
        Rc::new(move |word: &str| {
            let weak = weak.clone();
            let word = word.to_owned();
            glib::idle_add_local_once(move || {
                if let Some(this) = weak.upgrade() {
                    this.show_word(&word);
                }
            });
        })
    }

    fn search_changed(&self) {
        let text = self.search.text();
        if text.trim().is_empty() {
            self.suggestions.replace(Default::default());
            self.stack.set_visible_child_name(self.current_page.get());
            return;
        }
        let words = match self.source.suggestions(&text, SUGGESTIONS) {
            Ok(words) => words,
            Err(error) => {
                self.show_problem(&error);
                self.stack.set_visible_child_name(UNAVAILABLE);
                return;
            }
        };
        self.suggestion_list.remove_all();
        for word in &words {
            self.suggestion_list.append(&suggestion_row(word));
        }
        if words.is_empty() {
            self.suggestion_list.append(&no_suggestion_row(&text));
        }
        let selection = Selection::new(words.len(), None);
        self.suggestions.replace((words, selection));
        self.suggestion_list.unselect_all();
        self.stack.set_visible_child_name(SUGGESTIONS_PAGE);
    }

    /// Enter in the search field: the selected suggestion, else the word of
    /// `search_target`.
    fn search_activated(&self) {
        let selected = {
            let suggestions = self.suggestions.borrow();
            suggestions
                .1
                .selected()
                .and_then(|index| suggestions.0.get(index).cloned())
        };
        let target = selected.or_else(|| search_target(self.source.as_ref(), &self.search.text()));
        if let Some(word) = target {
            self.show_word(&word);
        }
    }

    fn key(&self, intent: Intent) {
        let suggesting = self.stack.visible_child_name().as_deref() == Some(SUGGESTIONS_PAGE);
        match intent {
            Intent::FocusSearch => {
                self.search.grab_focus();
                self.search.select_region(0, -1);
            }
            Intent::Back => self.go(History::back),
            Intent::Forward => self.go(History::forward),
            Intent::Escape => match keys::escape(&self.search.text()) {
                Escape::ClearField => self.search.set_text(""),
                Escape::Close => self.window.close(),
            },
            Intent::Up | Intent::Down if suggesting => self.move_selection(intent == Intent::Down),
            Intent::Up => self.scroll(-SCROLL_STEP),
            Intent::Down => self.scroll(SCROLL_STEP),
            Intent::PageUp => self.scroll(-self.page_size()),
            Intent::PageDown => self.scroll(self.page_size()),
        }
    }

    fn move_selection(&self, down: bool) {
        let selected = {
            let mut suggestions = self.suggestions.borrow_mut();
            if down {
                suggestions.1.down();
            } else {
                suggestions.1.up();
            }
            suggestions.1.selected()
        };
        let row = selected
            .and_then(|index| i32::try_from(index).ok())
            .and_then(|index| self.suggestion_list.row_at_index(index));
        match row {
            Some(row) => self.suggestion_list.select_row(Some(&row)),
            None => self.suggestion_list.unselect_all(),
        }
    }

    fn scroller(&self) -> Option<gtk::ScrolledWindow> {
        self.stack
            .visible_child()
            .and_downcast::<gtk::ScrolledWindow>()
    }

    fn page_size(&self) -> f64 {
        self.scroller()
            .map_or(0.0, |scroller| scroller.vadjustment().page_size() * 0.9)
    }

    fn scroll(&self, delta: f64) {
        if let Some(scroller) = self.scroller() {
            let adjustment = scroller.vadjustment();
            adjustment.set_value(adjustment.value() + delta);
        }
    }
}

fn scroller(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(child)
        .build()
}

fn suggestion_row(word: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::builder()
        .label(word)
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    gtk::ListBoxRow::builder()
        .child(&label)
        .focusable(false)
        .css_classes(["dictionary-suggestion"])
        .build()
}

fn no_suggestion_row(text: &str) -> gtk::ListBoxRow {
    let label = gtk::Label::builder()
        .label(format!(
            "No word starts with “{}”. Press Enter to see similar words.",
            text.trim()
        ))
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    gtk::ListBoxRow::builder()
        .child(&label)
        .focusable(false)
        .activatable(false)
        .selectable(false)
        .css_classes(["dictionary-suggestion"])
        .build()
}

/// The page for missing or bad data: the cause, the path and the command,
/// in labels that the owner can select and copy.
fn unavailable_page() -> (adw::StatusPage, ProblemLabels) {
    let line = |class: &str| {
        gtk::Label::builder()
            .selectable(true)
            .wrap(true)
            .wrap_mode(gtk::pango::WrapMode::WordChar)
            .justify(gtk::Justification::Center)
            .css_classes([class])
            .build()
    };
    let caption = |text: &str| {
        gtk::Label::builder()
            .label(text)
            .css_classes(["dim-label", "caption-heading"])
            .margin_top(18)
            .build()
    };
    let labels = ProblemLabels {
        cause: line("body"),
        path: line("monospace"),
        command: line("monospace"),
    };
    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .build();
    content.append(&labels.cause);
    content.append(&caption("Path that was tried"));
    content.append(&labels.path);
    content.append(&caption("Install the data with"));
    content.append(&labels.command);
    let hint = gtk::Label::builder()
        .label("Or set dictionary.wordnet_path in settings.toml to a WordNet directory.")
        .wrap(true)
        .justify(gtk::Justification::Center)
        .selectable(true)
        .css_classes(["dim-label"])
        .margin_top(18)
        .build();
    content.append(&hint);
    let page = adw::StatusPage::builder()
        .icon_name("accessories-dictionary-symbolic")
        .child(
            &adw::Clamp::builder()
                .maximum_size(520)
                .child(&content)
                .build(),
        )
        .build();
    (page, labels)
}

/// "Run in background" is off: the process stops when no window of the
/// program shows after `STOP_DELAY`.
fn stop_later(window: &adw::ApplicationWindow) {
    let Some(app) = window.application() else {
        return;
    };
    glib::timeout_add_local_once(STOP_DELAY, move || {
        if app.windows().iter().any(|window| window.is_visible()) {
            return;
        }
        log::info("\"Run in background\" is off: stop");
        app.quit();
    });
}

/// A signal handler that calls `f` while the window exists.
fn with<W: ?Sized>(
    weak: &Weak<DictionaryWindow>,
    f: impl Fn(&DictionaryWindow) + 'static,
) -> impl Fn(&W) + 'static {
    let weak = weak.clone();
    move |_| {
        if let Some(this) = weak.upgrade() {
            f(&this);
        }
    }
}
