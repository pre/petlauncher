# petlauncher-apps

Reads the desktop entries of the installed applications and of the settings pages, and gives a ranked search over them with usage counts. It also gives the start data of an entry, the list of web browsers, and a cheap change check for a reload in the background. It has no GTK, GLib, GNOME or D-Bus dependency. It does not start processes.

The directory rule, the filter rule, the settings page rule, and the match and rank rule: [docs/applications.md](../../docs/applications.md)

Test: `TMPDIR=$HOME/tmp cargo test -p petlauncher-apps`

Read-only check against the directories of this desktop: `cargo run -p petlauncher-apps --example list -- te gc wifi`
