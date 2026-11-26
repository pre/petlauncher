# petlauncher-core

The core of petlauncher: the contract with the user interface, the query engine, the path mode, the keys on a row, the shortcut rows and the kept query. It has no GTK, GLib, GNOME or D-Bus dependency. It does not start processes. It gives `Action` values, and the program executes them.

The contract, the mode of a query, the path mode rules and the threads: [docs/core.md](../../docs/core.md)

Test: `TMPDIR=$HOME/tmp cargo test -p petlauncher-core`
