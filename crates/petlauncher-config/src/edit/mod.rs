//! Change `settings.toml` and `shortcuts.toml` and keep the owner's
//! comments, spaces and order of items.

mod format;
mod settings;
mod shortcuts;

pub use settings::SettingsEditor;
pub use shortcuts::ShortcutsEditor;
