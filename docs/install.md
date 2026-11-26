# Installation

petlauncher installs for one user, with no root. The target is GNOME Shell 46 on Wayland (Ubuntu 24.04).

## Requirements

- Rust and cargo.
- The packages `libgtk-4-dev` and `libadwaita-1-dev` (GTK 4.14 and libadwaita 1.5).
- GNOME Shell 46. The extension of petlauncher is for this version only.

## Install

```sh
scripts/install.sh
```

The script builds the release binary and installs three items below the prefix `~/.local`:

| Item | Path |
|---|---|
| The program | `~/.local/bin/petlauncher` |
| The desktop entry | `~/.local/share/applications/pet.repo.Petlauncher.desktop` |
| The GNOME Shell extension | `~/.local/share/gnome-shell/extensions/petlauncher@repo.pet` |

`scripts/install.sh --prefix DIR` installs below a different prefix, for example a directory below `$HOME/tmp` for a test. The prefix must have no spaces.

The script changes no settings. Do these steps by hand after it:

1. Log out and log in, so that GNOME Shell finds the new extension.
2. Enable the extension: `gnome-extensions enable petlauncher@repo.pet`
3. Start petlauncher one time: `~/.local/bin/petlauncher`, or select petlauncher in the applications of GNOME.

## First start

At the first start petlauncher does these things:

- **Global key (R4).** When GNOME has no custom shortcut of petlauncher, petlauncher writes the custom shortcut "Show petlauncher" with the key of the setting `hotkey` (initial value Super+Space) and the command `petlauncher toggle`. It writes the shortcut only when no other GNOME key binding has the key. petlauncher never changes a different key binding.
- **Conflict.** When a different key binding has the key, petlauncher writes nothing and the launcher window shows a notice. The notice gives the name of the binding and its place in GNOME Settings > Keyboard > View and Customize Shortcuts. Remove the key there, then open the launcher again, for example with the top bar icon. GNOME gives Super+Space to "Switch to next input source" (section Typing) by default.
- **One time only.** After the shortcut is written, petlauncher writes the file `~/.local/state/petlauncher/global-key-installed`. While the file exists, petlauncher does not write the shortcut again. Thus a key that you remove in GNOME Settings stays removed. Delete the file to do the first installation again at the next start.
- **Start at login (R3).** With "Run in background" on, petlauncher writes `~/.config/autostart/pet.repo.Petlauncher.desktop` with the path of the program and `--hidden`, and starts at each login with no window. With the setting off, petlauncher removes the file.

The command `petlauncher toggle` of the global key needs `~/.local/bin` in the `PATH` of the GNOME session. On the owner's computer on 2026-11-26, the `PATH` of `gsd-media-keys` had `~/.local/bin`.

## Update

Run `scripts/install.sh` again. Quit petlauncher with its top bar menu and start it again. A change of the extension has effect after the next log in.

## Uninstall

```sh
scripts/uninstall.sh
```

The script removes the three installed items and the autostart file. It keeps the settings (`~/.config/petlauncher`), the cache (`~/.cache/petlauncher`) and the state (`~/.local/state/petlauncher`). It changes no GNOME settings. Do these steps by hand:

1. Quit petlauncher with its top bar menu.
2. Disable the extension: `gnome-extensions disable petlauncher@repo.pet`, then log out and log in.
3. Remove the custom shortcut "Show petlauncher" in GNOME Settings > Keyboard > View and Customize Shortcuts > Custom Shortcuts.
