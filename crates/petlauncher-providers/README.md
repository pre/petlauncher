# petlauncher-providers

One provider of `petlauncher-core` for each search crate (calculator, time, applications, settings pages, file search, dictionary), and the `Launcher` object that the program holds. The `Launcher` owns the settings, the engine, the kept query, the usage counts and the background threads. Plain Rust, with no GTK, GLib or D-Bus dependency.

The surface, the rows of each provider, the threads and the background rules: [docs/launcher.md](../../docs/launcher.md)

- Tests: `TMPDIR=$HOME/tmp cargo test -p petlauncher-providers`
- Rows for queries, with the real directories of the user: `cargo run --release -p petlauncher-providers --example query -- fire "f palette-zoom"`
