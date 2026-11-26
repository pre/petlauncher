# The GNOME Shell extension uses only the GApplication interface of the program

Status: accepted.

On GNOME Wayland a program cannot set the position of its window, so a GNOME Shell extension moves the launcher window. The extension moves the window whose Wayland application ID is `pet.repo.Petlauncher` and whose title is exactly `petlauncher`. For the top bar icon it calls the actions `show`, `settings` and `quit`, and reads the boolean state of `tray-icon`, through `org.gtk.Actions`. Reason: the program needs no D-Bus interface of its own, and a test on GNOME Shell 46 showed that the title and the application ID are present when the window shows.

## Considered Options

- The GTK window object path `/pet/repo/Petlauncher/window/N`: `N` depends on the order in which the program makes its windows.
- An own D-Bus handshake between the program and the extension: more code in the program and in the extension.
- A StatusNotifierItem tray icon: the target desktop has no `StatusNotifierWatcher`, so the icon does not show.

## Consequences

- The launcher window must have its title before it shows, and no other window can have that title.
- The extension must be examined again for each new GNOME Shell version.
