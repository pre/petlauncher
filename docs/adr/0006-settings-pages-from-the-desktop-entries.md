# Settings pages come from the desktop entries with a rule of the desktop part

Status: accepted.

The settings pages of the desktop are desktop entries. The `Launcher` reads them from the same `Applications` list as the applications, and the desktop part gives only the rule that marks an entry as a settings page (`Hooks::settings_page`, on GNOME `petlauncher_apps::gnome::is_settings_page`). One search with one rank and one usage count covers the two kinds, and a typing error shows only when neither kind matches without one.

## Considered Options

- `Desktop::settings_pages()` with the provider `SettingsPages` of the core: a second list with a weaker match (no typing errors, no accent removal and no usage points), and a second read of the same files.

## Consequences

- The `Launcher` does not use `Desktop::settings_pages` or `petlauncher_core::SettingsPages`. They are only for a desktop whose settings pages are not desktop entries.
