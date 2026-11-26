# petlauncher-gnome

The GNOME part of petlauncher. It implements the `Desktop` trait of `petlauncher-core` for GNOME 46: the file manager (R1, R2), the system commands (R16), the global key as a GNOME custom shortcut (R4) and the action for the top bar icon (R3). Its function `execute` does the desktop side of an `Action`. It uses GIO, not GTK.

The D-Bus calls, the GSettings keys, the evidence and the manual check list are in [docs/gnome-desktop.md](../../docs/gnome-desktop.md).

Test: `TMPDIR=$HOME/tmp cargo test -p petlauncher-gnome`. The tests use private D-Bus buses, the memory GSettings backend and isolated child processes. They do not touch the desktop session.
