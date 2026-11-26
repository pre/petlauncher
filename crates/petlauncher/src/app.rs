//! The application: one instance on the session bus, its D-Bus actions for
//! the GNOME Shell extension (`docs/gnome-extension.md`), and the commands
//! of a second start such as `petlauncher toggle`.

use std::cell::OnceCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use petlauncher_config::{ConfigFile, ConfigWatcher, Reload};

use crate::backend::Backend;
use crate::cli::{self, Command};
use crate::log;
use crate::settings_window;
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
    app.connect_startup(move |app| {
        let _ = at_startup.set(Program::start(app));
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
    tray_icon: gio::SimpleAction,
    _watcher: Option<ConfigWatcher>,
    /// The process stays after the window hides. When "Run in background"
    /// is off, `LauncherWindow::hide` stops it.
    _hold: gio::ApplicationHoldGuard,
}

impl Program {
    fn start(app: &adw::Application) -> Rc<Self> {
        load_style();
        let config_dir = petlauncher_config::config_dir()
            .unwrap_or_else(|| glib::user_config_dir().join("petlauncher"));
        let (backend, results) = Backend::new(config_dir, glib::home_dir());
        let backend = Rc::new(backend);
        let settings = backend.settings();
        let launcher = LauncherWindow::new(app, Rc::clone(&backend), results);

        // The extension reads the state and shows the icon when it is
        // true. Nobody can activate the action.
        let tray_icon = gio::SimpleAction::new_stateful(
            "tray-icon",
            None,
            &settings.show_tray_icon.to_variant(),
        );
        tray_icon.set_enabled(false);
        app.add_action(&tray_icon);

        let (changes, changed) = async_channel::unbounded();
        let watcher = ConfigWatcher::start(&backend.config_dir(), move |file| {
            let _ = changes.send_blocking(file);
        })
        .inspect_err(|error| log::info(format_args!("settings are not watched: {error}")))
        .ok();

        let program = Rc::new(Self {
            app: app.clone(),
            backend,
            launcher,
            tray_icon,
            _watcher: watcher,
            _hold: app.hold(),
        });
        program.add_actions();
        let weak = Rc::downgrade(&program);
        glib::spawn_future_local(async move {
            while let Ok(file) = changed.recv().await {
                match weak.upgrade() {
                    Some(program) => program.reload(file),
                    None => break,
                }
            }
        });
        log::info(format_args!(
            "started, version {}",
            env!("CARGO_PKG_VERSION")
        ));
        program
    }

    fn add_actions(self: &Rc<Self>) {
        let launcher = Rc::clone(&self.launcher);
        let show = gio::ActionEntry::builder("show")
            .activate(move |_: &adw::Application, _, _| launcher.show())
            .build();
        let config_dir = self.backend.config_dir();
        let settings = gio::ActionEntry::builder("settings")
            .activate(move |app: &adw::Application, _, _| {
                settings_window::present(app, &config_dir);
            })
            .build();
        let quit = gio::ActionEntry::builder("quit")
            .activate(|app: &adw::Application, _, _| app.quit())
            .build();
        self.app.add_action_entries([show, settings, quit]);
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

    fn reload(&self, file: ConfigFile) {
        if self.backend.reload(file) != Reload::Changed || file != ConfigFile::Settings {
            return;
        }
        let settings = self.backend.settings();
        self.launcher.apply_settings(&settings);
        self.tray_icon
            .set_state(&settings.show_tray_icon.to_variant());
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
