//! The application: one instance on the session bus, its D-Bus actions for
//! the GNOME Shell extension (`docs/gnome-extension.md`), the commands of a
//! second start such as `petlauncher toggle`, and the updates of the
//! `Launcher`.

use std::cell::{Cell, OnceCell, RefCell};
use std::rc::{Rc, Weak};

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use petlauncher_core::config::CustomPosition;
use petlauncher_core::Desktop;
use petlauncher_gnome::GnomeDesktop;
use petlauncher_providers::Update;

use crate::backend::{Backend, Events};
use crate::cli::{self, Command};
use crate::dictionary_window;
use crate::global_key::{self, FirstInstall};
use crate::log;
use crate::settings_window::{Host, Page, SettingsWindow};
use crate::window::LauncherWindow;

pub const APP_ID: &str = "pet.repo.Petlauncher";

/// Runs the program with the full command line. When an instance runs
/// already, GApplication sends the command line to it and this process
/// stops after that instance did the command.
pub fn run(args: &[String]) -> glib::ExitCode {
    // HANDLES_COMMAND_LINE with no GApplication options: GLib gives the
    // arguments unchanged to the running instance, and `cli` parses them.
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    let program: Rc<OnceCell<Rc<Program>>> = Rc::default();
    let at_startup = Rc::clone(&program);
    app.connect_startup(move |app| match Program::start(app) {
        Ok(started) => {
            let _ = at_startup.set(started);
        }
        Err(error) => {
            log::info(format_args!("petlauncher cannot start: {error}"));
            app.quit();
        }
    });
    app.connect_command_line(move |_, command_line| match program.get() {
        Some(program) => program.command(command_line),
        None => glib::ExitCode::FAILURE,
    });
    app.run_with_args(args)
}

struct Program {
    app: adw::Application,
    backend: Rc<Backend>,
    launcher: Rc<LauncherWindow>,
    /// R4: the first installation of the global key is not done yet.
    global_key_pending: Cell<bool>,
    /// The last problems of the background work that went to the log.
    logged_problems: RefCell<Vec<String>>,
    /// The settings window, made when it opens the first time.
    settings_window: RefCell<Option<Rc<SettingsWindow>>>,
    /// The process stays after the window hides. When "Run in background"
    /// is off, `LauncherWindow::hide` stops it.
    _hold: gio::ApplicationHoldGuard,
}

impl Program {
    fn start(app: &adw::Application) -> std::io::Result<Rc<Self>> {
        load_style();
        let desktop = GnomeDesktop::new();
        match &desktop {
            Ok(desktop) => {
                desktop.add_actions(app);
                if let Some(display) = gdk::Display::default() {
                    desktop.set_launch_context(Some(display.app_launch_context().upcast()));
                }
            }
            Err(error) => log::info(format_args!("no GNOME desktop: {error}")),
        }
        let (backend, Events { results, updates }) = Backend::start(desktop)?;
        let backend = Rc::new(backend);
        let launcher = LauncherWindow::new(app, Rc::clone(&backend), results);
        let background = Rc::clone(&backend);
        dictionary_window::install(app, Rc::new(backend.dictionary()), move || {
            background.settings().run_in_background
        });

        let program = Rc::new(Self {
            app: app.clone(),
            backend,
            launcher,
            global_key_pending: Cell::new(true),
            logged_problems: RefCell::default(),
            settings_window: RefCell::default(),
            _hold: app.hold(),
        });
        program.add_actions();
        program.apply_settings();
        program.install_global_key();
        let weak = Rc::downgrade(&program);
        program.launcher.set_before_show(move || {
            if let Some(program) = weak.upgrade() {
                program.install_global_key();
            }
        });
        let weak = Rc::downgrade(&program);
        program.launcher.set_on_pin(move |x, y| {
            if let Some(program) = weak.upgrade() {
                program.pin_position(CustomPosition { x, y });
            }
        });
        let weak = Rc::downgrade(&program);
        program.launcher.set_after_hide(move || {
            if let Some(program) = weak.upgrade() {
                program.publish_kept_query();
            }
        });
        let weak = Rc::downgrade(&program);
        glib::spawn_future_local(async move {
            while let Ok(update) = updates.recv().await {
                match weak.upgrade() {
                    Some(program) => program.update(update),
                    None => break,
                }
            }
        });
        log::info(format_args!(
            "started, version {}",
            env!("CARGO_PKG_VERSION")
        ));
        Ok(program)
    }

    fn add_actions(self: &Rc<Self>) {
        let launcher = Rc::clone(&self.launcher);
        let show = gio::ActionEntry::builder("show")
            .activate(move |_: &adw::Application, _, _| launcher.show())
            .build();
        let weak = Rc::downgrade(self);
        let settings = gio::ActionEntry::builder("settings")
            .activate(move |_: &adw::Application, _, _| {
                if let Some(program) = weak.upgrade() {
                    program.present_settings(None);
                }
            })
            .build();
        let weak = Rc::downgrade(self);
        let settings_page = gio::ActionEntry::builder("settings-page")
            .parameter_type(Some(glib::VariantTy::STRING))
            .activate(move |_: &adw::Application, _, parameter| {
                let page = parameter.and_then(|name| name.str().and_then(Page::from_name));
                if let Some(program) = weak.upgrade() {
                    program.present_settings(page);
                }
            })
            .build();
        // ADR 0009: the extension tells a move of the launcher by the owner,
        // with the place as fractions of the work area.
        let weak = Rc::downgrade(self);
        let launcher_moved = gio::ActionEntry::builder("launcher-moved")
            .parameter_type(Some(glib::VariantTy::new("(dd)").expect("a valid type")))
            .activate(move |_: &adw::Application, _, parameter| {
                let place = parameter.and_then(|place| place.get::<(f64, f64)>());
                if let (Some(program), Some((x, y))) = (weak.upgrade(), place) {
                    program.launcher.moved(x, y);
                }
            })
            .build();
        let quit = gio::ActionEntry::builder("quit")
            .activate(|app: &adw::Application, _, _| app.quit())
            .build();
        self.app
            .add_action_entries([show, settings, settings_page, launcher_moved, quit]);
    }

    fn command(&self, command_line: &gio::ApplicationCommandLine) -> glib::ExitCode {
        let arguments = command_line.arguments().into_iter().skip(1);
        let command = cli::parse(arguments);
        log::debug(format_args!(
            "command {command:?}, remote {}",
            command_line.is_remote()
        ));
        match command {
            Ok(Command::Start { hidden: false }) => self.launcher.show(),
            Ok(Command::Start { hidden: true }) => {
                if !command_line.is_remote() && !self.backend.settings().run_in_background {
                    log::info("\"Run in background\" is off: --hidden stops at once");
                    self.app.quit();
                }
            }
            Ok(Command::Toggle) => self.launcher.toggle(),
            Ok(other) => {
                command_line.printerr_literal(&format!(
                    "{other:?} is not a command of the running program\n"
                ));
                return glib::ExitCode::FAILURE;
            }
            Err(error) => {
                command_line.printerr_literal(&format!("{error}"));
                return glib::ExitCode::FAILURE;
            }
        }
        glib::ExitCode::SUCCESS
    }

    /// An update of the `Launcher`: the rows can be different now. After a
    /// change of a settings file, the new settings and the errors show.
    fn update(&self, update: Update) {
        log::debug(format_args!("update {update:?}"));
        if let Update::Config(_) = update {
            self.apply_settings();
            self.publish_kept_query();
            if let Some(window) = self.settings_window.borrow().as_ref() {
                window.reload();
            }
        }
        self.launcher.refresh();
        let problems = self.backend.problems();
        if *self.logged_problems.borrow() != problems {
            for problem in &problems {
                log::info(format_args!("problem: {problem}"));
            }
            self.logged_problems.replace(problems);
        }
    }

    /// Shows the settings window, with `page` or with the page that it
    /// showed last. There is one window for the process.
    fn present_settings(self: &Rc<Self>, page: Option<Page>) {
        let window = Rc::clone(self.settings_window.borrow_mut().get_or_insert_with(|| {
            let host: Weak<dyn Host> = Rc::downgrade(self) as Weak<Program>;
            SettingsWindow::new(&self.app, Rc::clone(&self.backend), host)
        }));
        window.present(page);
    }

    /// The settings that the program applies itself: the width of the
    /// window (R5), the tray icon (R3) and the notice of a settings file
    /// with an error (R18).
    fn apply_settings(&self) {
        let settings = self.backend.settings();
        self.launcher.apply_settings(&settings);
        if let Ok(desktop) = self.backend.desktop() {
            let _ = desktop.set_tray_icon_visible(settings.show_tray_icon);
            desktop.set_window_position(settings.window_position, settings.custom_position);
        }
        let notice = self.backend.config_notice();
        if let Some(notice) = &notice {
            log::info(notice);
        }
        self.launcher.set_config_notice(notice);
    }

    /// ADR 0009: the pin button. The place goes to `settings.toml` and to
    /// the state of `window-position`.
    fn pin_position(&self, place: CustomPosition) {
        log::info(format_args!("pinned the place {place:?}"));
        match self
            .backend
            .launcher()
            .edit_settings(|editor| editor.pin_position(place))
        {
            Ok(()) => self.apply_settings(),
            Err(error) => {
                log::info(format_args!("the place is not saved: {error}"));
                self.launcher
                    .show_error(format!("The place is not saved: {error}"));
            }
        }
    }

    /// ADR 0009: tells the extension until when the next show has the kept
    /// query, after each hide and each change of the settings. Not at a
    /// show: the extension reads the state when the window maps, and D-Bus
    /// can send a change of the state after the map.
    fn publish_kept_query(&self) {
        let until = self.backend.kept_query_until();
        log::debug(format_args!("kept query until {until:?}"));
        if let Ok(desktop) = self.backend.desktop() {
            desktop.set_kept_query_until(until);
        }
    }

    /// R4: the first installation of the global key, at the start and at
    /// each show until it is done. With a conflict the launcher shows a
    /// notice, and nothing is written.
    fn install_global_key(&self) {
        if !self.global_key_pending.get() {
            return;
        }
        let shortcuts = match self.backend.desktop().and_then(GnomeDesktop::shortcuts) {
            Ok(shortcuts) => shortcuts,
            Err(error) => {
                log::info(format_args!("no global key: {error}"));
                self.global_key_pending.set(false);
                return;
            }
        };
        log::debug(format_args!(
            "global key: the GSettings backend is {}",
            gio::SettingsBackend::default().type_().name()
        ));
        let hotkey = self.backend.settings().hotkey.clone();
        match global_key::install_once(shortcuts, &hotkey, self.backend.state_dir()) {
            Ok(result) => {
                let line = format!("global key: {}", result.log_line());
                if result == FirstInstall::Done {
                    log::debug(line);
                } else {
                    log::info(line);
                }
                self.launcher
                    .set_global_key_notice(result.notice(global_key::key_label));
                if !matches!(result, FirstInstall::Conflicts { .. }) {
                    self.global_key_pending.set(false);
                }
            }
            Err(error) => log::info(format_args!("global key: {error}")),
        }
    }
}

impl Host for Program {
    fn settings_saved(&self) {
        self.apply_settings();
    }

    fn global_key_changed(&self, key: Option<&str>) -> Result<(), String> {
        self.global_key_pending.set(false);
        self.launcher.set_global_key_notice(None);
        let mut errors = Vec::new();
        if let Err(error) = global_key::record_owner_choice(self.backend.state_dir(), key) {
            errors.push(error);
        }
        // The editor writes nothing when the setting has the key already.
        if let Err(error) = self
            .backend
            .launcher()
            .edit_settings(|editor| editor.set_hotkey(key.unwrap_or_default()))
        {
            errors.push(format!(
                "the setting hotkey in settings.toml was not changed: {error}"
            ));
        }
        for error in &errors {
            log::info(format_args!("global key: {error}"));
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
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
