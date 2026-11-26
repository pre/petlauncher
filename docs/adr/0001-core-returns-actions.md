# The core returns actions and does not call the desktop

Status: accepted.

petlauncher must work on a second desktop with a new desktop part only. Thus `petlauncher-core` and the leaf crates are plain Rust with no GTK, GLib, GNOME or D-Bus dependency. The core never starts a process, opens a URL or changes the clipboard: it returns an `Action` value, and the program executes it. Each function that depends on the desktop is a method of the trait `Desktop`, which `petlauncher-gnome` implements.

## Considered Options

- The core calls GIO directly: less code, but each test of a key or a row then needs a desktop session, and a KDE part cannot replace the GNOME calls.

## Consequences

- All behaviour of the keys, the path mode and the kept query has tests that need no display.
- A new row type needs a new `Action` variant and code in the program that executes it.
