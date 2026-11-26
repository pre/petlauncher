# GNOME Shell extension

The GNOME part of petlauncher for GNOME Shell 46 on Wayland (R11 and the tray icon of R3):

- It puts the launcher window at the center of the monitor with the pointer, each time the window shows.
- It shows the petlauncher icon in the top bar while the program runs and the setting `show_tray_icon` is on.

The contract with the program, the test method and the test results are in [docs/gnome-extension.md](../../docs/gnome-extension.md).

| Path | Contents |
|---|---|
| `petlauncher@repo.pet/` | The extension. `launcher.js` has the identifiers of the contract and the position rule. |
| `test/test-launcher.js` | Unit test of `launcher.js`: `gjs -m gnome/extension/test/test-launcher.js` |
| `test/nested-test.sh` | Runs the extension in a separate headless GNOME Shell with two monitors, and writes the evidence. |
| `test/fake-petlauncher.js` | A gjs GTK 4 stand-in for the program, with the same application ID, windows and actions. |
| `test/shell-probe.js` | The automation script of the test shell: it records the window positions and the painted frames. |
| `test/analyse.sh` | Makes the result table from the evidence of `nested-test.sh`. |

Install by hand: copy `petlauncher@repo.pet/` to `~/.local/share/gnome-shell/extensions/`, log out and log in, then run `gnome-extensions enable petlauncher@repo.pet`.
