# petlauncher

An application launcher, calculator and snippet manager for GNOME on Wayland. Rust with GTK 4 and libadwaita. Status: in development. The launcher works with all searches and the actions of the GNOME desktop, the dictionary rows open the dictionary window, and the settings window changes all settings, the shortcuts and the global key. Install: [docs/install.md](docs/install.md).

## Parts

| Path | Contents | Detail |
|---|---|---|
| `crates/petlauncher` | The program: command line, application, launcher window | [docs/program.md](docs/program.md) |
| `crates/petlauncher-core` | Contract, query engine, path mode, keys, kept query | [docs/core.md](docs/core.md) |
| `crates/petlauncher-config` | `settings.toml` and `shortcuts.toml` | [docs/settings.md](docs/settings.md) |
| `crates/petlauncher-calc` | Calculator, percent, units, currencies | [docs/calculator.md](docs/calculator.md) |
| `crates/petlauncher-time` | Time in other time zones | [docs/time-zones.md](docs/time-zones.md) |
| `crates/petlauncher-index` | File search by name | [docs/file-search.md](docs/file-search.md) |
| `crates/petlauncher-dict` | WordNet dictionary | [docs/dictionary.md](docs/dictionary.md) |
| `crates/petlauncher-apps` | Applications and settings pages | [docs/applications.md](docs/applications.md) |
| `crates/petlauncher-providers` | Providers for each search crate, and the `Launcher` that the program holds | [docs/launcher.md](docs/launcher.md) |
| `crates/petlauncher-gnome` | GNOME part: file manager, system commands, global key, execution of actions | [docs/gnome-desktop.md](docs/gnome-desktop.md) |
| `gnome/extension` | GNOME Shell extension: window position and top bar icon | [docs/gnome-extension.md](docs/gnome-extension.md) |

The decisions that are hard to reverse are in [docs/adr](docs/adr/README.md).

## Build, test and install

Install the packages `libgtk-4-dev` and `libadwaita-1-dev` and the command runner [just](https://github.com/casey/just). Then:

| Command | Function |
|---|---|
| `just build` | Builds the release program with cargo. |
| `just test` | Runs the Rust tests and the unit test of the extension. |
| `just lint` | Checks the format and the lints. |
| `just install` | Builds and installs for the current user, with no root. See [docs/install.md](docs/install.md). |
| `just uninstall` | Removes the installed files. |
| `just release v1.2.3` | Makes a draft GitHub release. See [docs/release.md](docs/release.md). |

## License

MIT, see [LICENSE](LICENSE).
