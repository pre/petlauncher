//! The dictionary window of R19: one window for the process. Enter on a
//! dictionary row of the launcher opens it with the word, and a new word
//! replaces the word in it. `docs/dictionary-window.md` describes the
//! window, the keys and the decisions.
//!
//! The program calls `install` one time at its start, and the launcher calls
//! `open` for `Action::OpenDictionary`. The window gets the words through
//! the trait `DictionarySource`.

mod history;
mod keys;
mod markup;
mod page;
mod source;
mod view;
mod window;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::gdk;

pub use source::{DictionarySource, WordNet};

use crate::log;
use window::DictionaryWindow;

/// The extension moves only the window with the title `petlauncher` (ADR
/// 0005), so it does not move this window.
pub const TITLE: &str = "petlauncher Dictionary";

struct Installed {
    app: adw::Application,
    source: Rc<dyn DictionarySource>,
    run_in_background: Rc<dyn Fn() -> bool>,
    /// The window is made at the first `open`.
    window: Option<Rc<DictionaryWindow>>,
}

thread_local! {
    static INSTALLED: RefCell<Option<Installed>> = const { RefCell::new(None) };
}

/// Prepares the window. `run_in_background` gives the setting now: when it
/// is off, the process stops after the dictionary window and the launcher
/// are both hidden.
pub fn install(
    app: &adw::Application,
    source: Rc<dyn DictionarySource>,
    run_in_background: impl Fn() -> bool + 'static,
) {
    load_style();
    INSTALLED.set(Some(Installed {
        app: app.clone(),
        source,
        run_in_background: Rc::new(run_in_background),
        window: None,
    }));
}

/// Shows the word in the dictionary window, and brings the window to the
/// front.
pub fn open(word: &str) {
    let window = INSTALLED.with_borrow_mut(|installed| {
        let installed = installed.as_mut()?;
        let window = installed.window.get_or_insert_with(|| {
            DictionaryWindow::new(
                &installed.app,
                Rc::clone(&installed.source),
                Rc::clone(&installed.run_in_background),
            )
        });
        Some(Rc::clone(window))
    });
    match window {
        Some(window) => {
            log::debug(format_args!("dictionary window: open {word:?}"));
            window.open(word);
        }
        None => log::info(format_args!(
            "dictionary window: not installed, word {word:?}"
        )),
    }
}

fn load_style() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(include_str!("style.css"));
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
