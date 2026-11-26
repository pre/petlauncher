//! The settings window: one window for the process, opened by the action
//! `settings` (tray menu) and by Ctrl+comma in the launcher.
//! `docs/settings-window.md` describes the pages and the decisions.
//!
//! The window changes the files only through the editors of the store
//! (R18): each change writes only its own value. The plain modules hold the
//! logic with no widgets: `edits` (a change and its editor calls), `form`
//! (one shortcut), `hotkey` (the global key) and `keep_time` (R9).

mod edits;
mod form;
mod form_page;
mod general_page;
mod hotkey;
mod keep_time;
mod key_page;
mod search_page;
mod shortcuts_page;
mod widgets;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::glib;
use petlauncher_config::{ConfigFile, EditError};
use petlauncher_providers::Launcher;

use crate::backend::Backend;
use crate::log;
use crate::stop;
use edits::{refused, SettingsEdit, ShortcutEdit};

/// The extension does not move a window with this title (ADR 0005).
pub const TITLE: &str = "petlauncher Settings";

/// What the window needs from the program.
pub trait Host {
    /// After a saved change of `settings.toml`: the program applies the
    /// settings that it owns, the width (R5) and the tray icon (R3).
    fn settings_saved(&self);

    /// After the window wrote the global key in GNOME, or removed it
    /// (`None`). The program writes the setting `hotkey`, ends the first
    /// installation (R4) and removes the notice in the launcher. The error
    /// is for the owner.
    fn global_key_changed(&self, key: Option<&str>) -> Result<(), String>;
}

/// The state that the pages share.
pub(crate) struct Context {
    pub window: adw::PreferencesWindow,
    pub backend: Rc<Backend>,
    host: Weak<dyn Host>,
    /// True while the window shows the values of the files. The signal
    /// handlers of the widgets then write nothing.
    pub loading: Cell<bool>,
    /// Shows the values again after a change.
    reload: RefCell<Option<Box<dyn Fn()>>>,
}

impl Context {
    pub fn launcher(&self) -> &Launcher {
        self.backend.launcher()
    }

    pub fn host(&self) -> Option<Rc<dyn Host>> {
        self.host.upgrade()
    }

    pub fn home(&self) -> PathBuf {
        glib::home_dir()
    }

    pub fn file(&self, file: ConfigFile) -> PathBuf {
        file.path_in(self.backend.config_dir())
    }

    /// Saves one change of `settings.toml`. The error text is for the
    /// owner; the window also shows it in a toast.
    pub fn save_settings(&self, edit: SettingsEdit) -> Result<(), String> {
        if self.loading.get() {
            return Ok(());
        }
        edit.check(&self.home())?;
        match self.launcher().edit_settings(|editor| edit.apply(editor)) {
            Ok(()) => {
                log::debug(format_args!("settings window: saved {edit:?}"));
                if let Some(host) = self.host() {
                    host.settings_saved();
                }
                self.reload_later();
                Ok(())
            }
            Err(error) => {
                let text = refused(&error);
                log::info(format_args!("settings window: not saved {edit:?}: {text}"));
                self.toast(&text);
                self.reload_later();
                Err(text)
            }
        }
    }

    /// Saves one change of `shortcuts.toml`. After `FileChanged` the store
    /// read the file again, and the window shows the new list.
    pub fn save_shortcuts(&self, edit: ShortcutEdit) -> Result<(), EditError> {
        let result = self.launcher().edit_shortcuts(|editor| edit.apply(editor));
        match &result {
            Ok(()) => log::debug(format_args!("settings window: saved {edit:?}")),
            Err(error) => log::info(format_args!(
                "settings window: not saved {edit:?}: {}",
                refused(error)
            )),
        }
        self.reload_later();
        result
    }

    pub fn toast(&self, text: &str) {
        let toast = adw::Toast::builder()
            .title(text)
            .use_markup(false)
            .timeout(6)
            .build();
        self.window.add_toast(toast);
    }

    /// Shows the values again in an idle, so that a handler does not remove
    /// its own widget.
    pub fn reload_later(&self) {
        if let Some(reload) = self.reload.borrow().as_ref() {
            reload();
        }
    }
}

/// The window and its pages.
pub struct SettingsWindow {
    context: Rc<Context>,
    general: general_page::GeneralPage,
    key: key_page::KeyPage,
    search: search_page::SearchPage,
    shortcuts: Rc<shortcuts_page::ShortcutsPage>,
}

impl SettingsWindow {
    pub fn new(app: &adw::Application, backend: Rc<Backend>, host: Weak<dyn Host>) -> Rc<Self> {
        let window = adw::PreferencesWindow::builder()
            .application(app)
            .title(TITLE)
            // The template of AdwPreferencesWindow makes it modal. A modal
            // window gets the keys of the launcher, so Escape in the
            // launcher would close this window.
            .modal(false)
            .destroy_with_parent(false)
            .default_width(720)
            .default_height(760)
            .hide_on_close(true)
            .build();
        let context = Rc::new(Context {
            window: window.clone(),
            backend,
            host,
            loading: Cell::new(false),
            reload: RefCell::default(),
        });
        let this = Rc::new(Self {
            general: general_page::GeneralPage::new(&context),
            key: key_page::KeyPage::new(&context),
            search: search_page::SearchPage::new(&context),
            shortcuts: shortcuts_page::ShortcutsPage::new(&context),
            context,
        });
        window.add(&this.general.page);
        window.add(&this.key.page);
        window.add(&this.search.page);
        window.add(&this.shortcuts.page);

        let weak = Rc::downgrade(&this);
        this.context.reload.replace(Some(Box::new(move || {
            let weak = weak.clone();
            glib::idle_add_local_once(move || {
                if let Some(window) = weak.upgrade() {
                    window.reload();
                }
            });
        })));
        let background = Rc::clone(&this.context.backend);
        window.connect_close_request(move |window| {
            log::debug("settings window: hide");
            if !background.settings().run_in_background {
                if let Some(app) = window.application() {
                    stop::stop_when_no_window_shows(&app);
                }
            }
            glib::Propagation::Proceed
        });
        this
    }

    /// Shows the window with the values of the files and the key of GNOME
    /// now (R4: the two places show the same key).
    pub fn present(&self) {
        self.key.load(&self.context);
        self.reload();
        self.context.window.present();
        log::debug("settings window: show");
    }

    /// Shows the values in effect. The program calls it after a change of a
    /// file in an editor (`Update::Config`).
    pub fn reload(&self) {
        self.context.loading.set(true);
        self.general.reload(&self.context);
        self.key.reload(&self.context);
        self.search.reload(&self.context);
        self.shortcuts.reload(&self.context);
        self.context.loading.set(false);
    }
}
