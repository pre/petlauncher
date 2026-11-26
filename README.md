# petlauncher

An application launcher, calculator and snippet manager for GNOME on Wayland. Rust with GTK 4 and libadwaita. Status: in development. The launcher window works with the path mode, the shortcuts and the system commands. The other searches, the desktop actions and the settings window are not joined yet. The dictionary window works, but the launcher gives no dictionary rows yet.

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

## Build

Install the packages `libgtk-4-dev` and `libadwaita-1-dev`, then run `cargo build -p petlauncher --release`.

## Test

```
cargo test --workspace
gjs -m gnome/extension/test/test-launcher.js
```

## License

MIT, see [LICENSE](LICENSE).
