# petlauncher

The program `petlauncher`: the command line, the application on the session bus and the launcher window, with GTK 4 and libadwaita. It joins the `Launcher` of `petlauncher-providers` and the GNOME desktop of `petlauncher-gnome` in `backend.rs`. The rules of the window are in plain modules with tests (`model`, `selection`, `keys`, `visibility`, `notices`, `worker`, `global_key`). The widgets are in `window.rs` and `rows.rs`. The dictionary window is the module `dictionary_window`, the settings window the module `settings_window`, which also has its rules in plain modules with tests.

Build: install `libgtk-4-dev` and `libadwaita-1-dev`, then `cargo build -p petlauncher --release`. Install for one user: [docs/install.md](../../docs/install.md).

The command line, the window, the threads and the test: [docs/program.md](../../docs/program.md)

The settings window: [docs/settings-window.md](../../docs/settings-window.md)

Test: `TMPDIR=$HOME/tmp cargo test -p petlauncher`
