//! The settings window: one window for the process, opened by the action
//! `settings` (tray menu) and by Ctrl+comma in the launcher. The pages are
//! in a list at the left edge. `docs/settings-window.md` describes the pages and
//! the decisions.
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

/// A page of the window, in the order of the list at the left edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    General,
    GlobalKey,
    Search,
    Shortcuts,
}

impl Page {
    pub const ALL: [Page; 4] = [
        Page::General,
        Page::GlobalKey,
        Page::Search,
        Page::Shortcuts,
    ];

    /// The name of the page in the list of pages, for example `global-key`.
    pub fn name(self) -> &'static str {
        match self {
            Page::General => "general",
            Page::GlobalKey => "global-key",
            Page::Search => "search",
            Page::Shortcuts => "shortcuts",
        }
    }
}

/// The state that the pages share.
pub(crate) struct Context {
    pub window: adw::ApplicationWindow,
    /// The pages, and above them a subpage such as the form of a shortcut.
    pub navigation: adw::NavigationView,
    toasts: adw::ToastOverlay,
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
        self.toasts.add_toast(toast);
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
    sidebar: gtk::ListBox,
    /// The navigation page of the list and the pages, below the subpages.
    root: adw::NavigationPage,
    general: general_page::GeneralPage,
    key: key_page::KeyPage,
    search: search_page::SearchPage,
    shortcuts: Rc<shortcuts_page::ShortcutsPage>,
}

impl SettingsWindow {
    pub fn new(app: &adw::Application, backend: Rc<Backend>, host: Weak<dyn Host>) -> Rc<Self> {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(TITLE)
            .default_width(900)
            .default_height(760)
            .hide_on_close(true)
            .build();
        let navigation = adw::NavigationView::new();
        let toasts = adw::ToastOverlay::new();
        toasts.set_child(Some(&navigation));
        window.set_content(Some(&toasts));
        let context = Rc::new(Context {
            window: window.clone(),
            navigation,
            toasts,
            backend,
            host,
            loading: Cell::new(false),
            reload: RefCell::default(),
        });
        let general = general_page::GeneralPage::new(&context);
        let key = key_page::KeyPage::new(&context);
        let search = search_page::SearchPage::new(&context);
        let shortcuts = shortcuts_page::ShortcutsPage::new(&context);

        let stack = gtk::Stack::new();
        let sidebar = gtk::ListBox::builder()
            .css_classes(["navigation-sidebar"])
            .build();
        for (page, widget) in
            Page::ALL
                .into_iter()
                .zip([&general.page, &key.page, &search.page, &shortcuts.page])
        {
            stack.add_named(widget, Some(page.name()));
            sidebar.append(&sidebar_row(widget));
        }
        let pages = adw::NavigationPage::builder()
            .title(general.page.title())
            .child(&with_header(&stack))
            .build();
        let split = adw::NavigationSplitView::builder()
            .sidebar(
                &adw::NavigationPage::builder()
                    .title("Settings")
                    .child(&with_header(
                        &gtk::ScrolledWindow::builder()
                            .hscrollbar_policy(gtk::PolicyType::Never)
                            .child(&sidebar)
                            .build(),
                    ))
                    .build(),
            )
            .content(&pages)
            .build();
        let root = adw::NavigationPage::builder()
            .title(TITLE)
            .child(&split)
            .build();
        context.navigation.add(&root);
        let shown = pages.clone();
        sidebar.connect_row_selected(move |_, row| {
            let Some(page) = row.and_then(|row| usize::try_from(row.index()).ok()) else {
                return;
            };
            stack.set_visible_child_name(Page::ALL[page].name());
            if let Some(child) = stack.visible_child().and_downcast::<adw::PreferencesPage>() {
                shown.set_title(&child.title());
            }
        });
        sidebar.select_row(sidebar.row_at_index(0).as_ref());

        let this = Rc::new(Self {
            context,
            sidebar,
            root,
            general,
            key,
            search,
            shortcuts,
        });
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
    /// now (R4: the two places show the same key). With `page`, the window
    /// shows that page, also when a subpage was open.
    pub fn present(&self, page: Option<Page>) {
        self.key.load(&self.context);
        self.reload();
        if let Some(page) = page {
            self.context.navigation.pop_to_page(&self.root);
            let row = self.sidebar.row_at_index(page as i32);
            self.sidebar.select_row(row.as_ref());
        }
        self.context.window.present();
        log::debug(format_args!("settings window: show, page {page:?}"));
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

/// A row of the list at the left edge: the icon and the full title of the
/// page.
fn sidebar_row(page: &adw::PreferencesPage) -> gtk::ListBoxRow {
    let line = gtk::Box::builder().spacing(12).build();
    line.append(&gtk::Image::from_icon_name(
        &page.icon_name().unwrap_or_default(),
    ));
    line.append(
        &gtk::Label::builder()
            .label(page.title())
            .xalign(0.0)
            .build(),
    );
    let row = gtk::ListBoxRow::builder().child(&line).build();
    row.update_property(&[gtk::accessible::Property::Label(&page.title())]);
    row
}

/// The child with a header bar above it. The header bar shows the title of
/// its navigation page.
fn with_header(child: &impl IsA<gtk::Widget>) -> adw::ToolbarView {
    let view = adw::ToolbarView::builder().content(child).build();
    view.add_top_bar(&adw::HeaderBar::new());
    view
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_order_is_the_order_of_the_enum() {
        for (index, page) in Page::ALL.into_iter().enumerate() {
            assert_eq!(page as usize, index);
        }
    }
}
