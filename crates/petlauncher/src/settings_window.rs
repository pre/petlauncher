//! The settings window. A later task writes it. Now the window has only a
//! short text, so that the action `settings` of the tray icon shows a
//! window.

use std::path::Path;

use adw::prelude::*;

/// The extension does not move a window with this title (ADR 0005).
pub const TITLE: &str = "petlauncher Settings";

pub fn present(app: &adw::Application, config_dir: &Path) {
    let existing = app
        .windows()
        .into_iter()
        .find(|window| window.title().as_deref() == Some(TITLE));
    if let Some(window) = existing {
        window.present();
        return;
    }
    let page = adw::StatusPage::builder()
        .icon_name("preferences-system-symbolic")
        .title("Settings")
        .description(format!(
            "The settings window is not written yet. \
             Edit settings.toml and shortcuts.toml in this folder:\n{}",
            gtk::glib::markup_escape_text(&config_dir.to_string_lossy())
        ))
        .build();
    let view = adw::ToolbarView::builder().content(&page).build();
    view.add_top_bar(&adw::HeaderBar::new());
    adw::ApplicationWindow::builder()
        .application(app)
        .title(TITLE)
        .default_width(600)
        .default_height(400)
        .hide_on_close(true)
        .content(&view)
        .build()
        .present();
}
