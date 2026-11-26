//! The page General: the switches of R3, the window width (R5), the time of
//! the kept query (R9) and the settings files (R18).

use std::rc::Rc;

use adw::prelude::*;
use petlauncher_config::validate::WINDOW_WIDTH;
use petlauncher_config::{ConfigFile, Severity};
use petlauncher_core::keep_query_warning;

use super::edits::SettingsEdit;
use super::keep_time::{KeepTime, KeepUnit};
use super::widgets::{open_button, FileErrorGroup, MessageRow};
use super::Context;

pub struct GeneralPage {
    pub page: adw::PreferencesPage,
    error: FileErrorGroup,
    editable: Vec<adw::PreferencesGroup>,
    close_on_focus_loss: adw::SwitchRow,
    run_in_background: adw::SwitchRow,
    show_tray_icon: adw::SwitchRow,
    include_applications: adw::SwitchRow,
    width: adw::SpinRow,
    keep_unit: adw::ComboRow,
    keep_number: adw::SpinRow,
    keep_warning: MessageRow,
}

impl GeneralPage {
    pub fn new(context: &Rc<Context>) -> Self {
        let page = adw::PreferencesPage::builder()
            .title("General")
            .icon_name("preferences-system-symbolic")
            .build();
        let error = FileErrorGroup::new(context, context.file(ConfigFile::Settings));
        page.add(&error.group);

        let behaviour = adw::PreferencesGroup::builder().title("Behaviour").build();
        let switch = |title: &str, subtitle: &str| {
            let row = adw::SwitchRow::builder()
                .title(title)
                .subtitle(subtitle)
                .build();
            behaviour.add(&row);
            row
        };
        let close_on_focus_loss = switch(
            "Close when losing focus",
            "Close the window when a different window gets the focus",
        );
        let run_in_background = switch(
            "Run in background",
            "Keep petlauncher running after the window closes, so the window shows quickly. \
             petlauncher then also starts at login.",
        );
        let show_tray_icon = switch("Show tray icon", "Show an icon in the top bar");
        let include_applications = switch(
            "Include applications in search",
            "Show the installed applications in the results",
        );
        page.add(&behaviour);

        let window = adw::PreferencesGroup::builder().title("Window").build();
        let width = adw::SpinRow::builder()
            .title("Width")
            .subtitle(format!(
                "In pixels, from {} to {}",
                WINDOW_WIDTH.start(),
                WINDOW_WIDTH.end()
            ))
            .adjustment(&gtk::Adjustment::new(
                750.0,
                f64::from(*WINDOW_WIDTH.start()),
                f64::from(*WINDOW_WIDTH.end()),
                10.0,
                50.0,
                0.0,
            ))
            .numeric(true)
            .build();
        window.add(&width);
        page.add(&window);

        let kept = adw::PreferencesGroup::builder()
            .title("Kept query")
            .description("How long the last query stays in the query line after the window closes")
            .build();
        let units = gtk::StringList::new(&KeepUnit::ALL.map(KeepUnit::label));
        let keep_unit = adw::ComboRow::builder()
            .title("Keep the query")
            .model(&units)
            .build();
        let keep_number = adw::SpinRow::builder()
            .title("Time")
            .adjustment(&gtk::Adjustment::new(30.0, 1.0, 86_400.0, 1.0, 10.0, 0.0))
            .numeric(true)
            .build();
        let keep_warning = MessageRow::new();
        kept.add(&keep_unit);
        kept.add(&keep_number);
        kept.add(&keep_warning.row);
        page.add(&kept);

        let files = adw::PreferencesGroup::builder()
            .title("Settings files")
            .description(
                "petlauncher keeps its settings in these text files. \
                 A change in a text editor takes effect at once.",
            )
            .build();
        for (title, path, label) in [
            (
                "settings.toml",
                context.file(ConfigFile::Settings),
                "Open File",
            ),
            (
                "shortcuts.toml",
                context.file(ConfigFile::Shortcuts),
                "Open File",
            ),
            (
                "Settings folder",
                context.backend.config_dir().to_owned(),
                "Open Folder",
            ),
        ] {
            let row = adw::ActionRow::builder()
                .title(title)
                .subtitle(path.display().to_string())
                .use_markup(false)
                .subtitle_selectable(true)
                .build();
            row.add_suffix(&open_button(context, label, path));
            files.add(&row);
        }
        page.add(&files);

        let this = Self {
            page,
            error,
            editable: vec![behaviour, window, kept],
            close_on_focus_loss,
            run_in_background,
            show_tray_icon,
            include_applications,
            width,
            keep_unit,
            keep_number,
            keep_warning,
        };
        this.connect(context);
        this
    }

    fn connect(&self, context: &Rc<Context>) {
        type Edit = fn(bool) -> SettingsEdit;
        let switches: [(&adw::SwitchRow, Edit); 4] = [
            (&self.close_on_focus_loss, SettingsEdit::CloseOnFocusLoss),
            (&self.run_in_background, SettingsEdit::RunInBackground),
            (&self.show_tray_icon, SettingsEdit::ShowTrayIcon),
            (
                &self.include_applications,
                SettingsEdit::IncludeApplications,
            ),
        ];
        for (row, edit) in switches {
            let context = Rc::downgrade(context);
            row.connect_active_notify(move |row| {
                if let Some(context) = context.upgrade() {
                    let _ = context.save_settings(edit(row.is_active()));
                }
            });
        }
        let weak = Rc::downgrade(context);
        self.width.connect_value_notify(move |row| {
            if let Some(context) = weak.upgrade() {
                let _ = context.save_settings(SettingsEdit::WindowWidth(row.value() as u32));
            }
        });
        let save_time = {
            let unit = self.keep_unit.clone();
            let number = self.keep_number.clone();
            let weak = Rc::downgrade(context);
            move || {
                let Some(context) = weak.upgrade() else {
                    return;
                };
                let time = KeepTime {
                    number: number.value() as u64,
                    unit: KeepUnit::from_index(unit.selected()),
                };
                number.set_sensitive(time.unit.has_number());
                let _ = context.save_settings(SettingsEdit::KeepQuery(time.to_keep_query()));
            }
        };
        let on_unit = save_time.clone();
        self.keep_unit.connect_selected_notify(move |_| on_unit());
        self.keep_number.connect_value_notify(move |_| save_time());
    }

    pub fn reload(&self, context: &Context) {
        let settings = context.backend.settings();
        let error = context.launcher().config_error(ConfigFile::Settings);
        self.error.set("settings.toml", error.as_ref());
        for group in &self.editable {
            group.set_sensitive(error.is_none());
        }
        self.close_on_focus_loss
            .set_active(settings.close_on_focus_loss);
        self.run_in_background
            .set_active(settings.run_in_background);
        self.show_tray_icon.set_active(settings.show_tray_icon);
        self.include_applications
            .set_active(settings.include_applications);
        self.width.set_value(f64::from(settings.window_width));

        let time = KeepTime::from(settings.keep_query);
        self.keep_unit.set_selected(time.unit.index());
        self.keep_number.set_value(time.number as f64);
        self.keep_number.set_subtitle(match time.unit {
            KeepUnit::Minutes => "In minutes",
            _ => "In seconds",
        });
        let warning = keep_query_warning(&settings);
        self.keep_warning
            .set(warning.map(|text| (Severity::Warning, text)));
        self.keep_unit.set_sensitive(warning.is_none());
        self.keep_number
            .set_sensitive(warning.is_none() && time.unit.has_number());
    }
}
