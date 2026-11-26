# GNOME Shell extension

The GNOME part of petlauncher for GNOME Shell 46 on Wayland (R11 and the tray icon of R3):

- It puts the launcher window at the center of the monitor with the pointer, each time the window shows. The query line has the same place for each height of the window.
- It shows the petlauncher icon in the top bar while the program runs and the setting `show_tray_icon` is on.

The contract with the program, the test method and the test results are in [docs/gnome-extension.md](../../docs/gnome-extension.md).

| Path | Contents |
|---|---|
| `petlauncher@repo.pet/` | The extension. `launcher.js` has the identifiers of the contract and the position rule. |
| `test/test-launcher.js` | Unit test of `launcher.js`: `gjs -m gnome/extension/test/test-launcher.js` |
| `test/nested-test.sh` | Runs the extension in a separate headless GNOME Shell with two monitors, and writes the evidence. With `--program PATH` it tests the real program. |
| `test/fake-petlauncher.js` | A gjs GTK 4 stand-in for the program, with the same application ID, windows and actions. |
| `test/shell-probe.js` | The automation script of the test shell: it moves the pointer, types, reads the clipboard, and records the window positions and the painted frames. |
| `test/analyse.sh` | Makes the result table from the evidence of `nested-test.sh`. |
| `test/program-steps.sh` | The steps of `nested-test.sh --program`. |
| `test/program-files.sh` | Writes the settings and the test files of those steps. |
| `test/analyse-program.sh` | Makes the tables of the query line, the times and the actions of those steps. |

Install by hand: copy `petlauncher@repo.pet/` to `~/.local/share/gnome-shell/extensions/`, log out and log in, then run `gnome-extensions enable petlauncher@repo.pet`.
