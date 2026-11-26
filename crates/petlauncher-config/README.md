# petlauncher-config

Reads, checks and writes the two settings files of petlauncher,
`settings.toml` and `shortcuts.toml`. Plain Rust, with no GTK, GLib or D-Bus.

- `ConfigStore` holds the values in effect and the last error of each file,
  and writes the changes of the settings window.
- `ConfigWatcher` reports a file that the owner saved. Give each report to
  `ConfigStore::reload`.
- `convert` converts the Ulauncher shortcuts one time.

The keys, the initial values and the rules for errors and warnings are in
[docs/settings.md](../../docs/settings.md).
