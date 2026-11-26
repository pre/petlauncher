# The program

The crate `crates/petlauncher` is the program `petlauncher`: the command line, the application on the session bus and the launcher window. It uses GTK 4 and libadwaita. This file describes the command line, the window, the threads, the seam for the next parts, the build and the test.

The settings window and the dictionary window are not written yet. The program has a place for each of them (see "Seam for the next parts").

## Build

Install the packages `libgtk-4-dev` and `libadwaita-1-dev`, then:

```sh
cargo build -p petlauncher --release
```

The crate uses gtk4 0.11 with the feature `gnome_46` (GTK 4.14, GLib 2.80, Pango 1.52) and libadwaita 0.9 with the feature `v1_5`. These are the versions of Ubuntu 24.04. A newer feature needs a newer runtime, so do not select one.

## Command line

| Command | Function |
|---|---|
| `petlauncher` | Starts the program and shows the launcher. When the program runs already, it shows its launcher. |
| `petlauncher --hidden` | Starts the program with no window. This is the command for the start at login. When the program runs already, nothing changes. When "Run in background" is off, the program stops at once. |
| `petlauncher toggle` | Shows the launcher, or hides it when it shows. When the program does not run, this process starts it and shows the launcher. The global key runs this command (R4). |
| `petlauncher convert-shortcuts [--force]` | Writes the shortcuts of `~/.config/ulauncher/shortcuts.json` to `shortcuts.toml` and prints a summary (R6). Without `--force`, an existing `shortcuts.toml` stops the command. |
| `petlauncher --version` | Prints the version. |
| `petlauncher --help` | Prints the commands. |

The exit status is 0 on success, 1 when `convert-shortcuts` fails, and 2 for a wrong argument.

One instance of the program runs in each session. It is a `GApplication` with the ID `pet.repo.Petlauncher` and the flag `HANDLES_COMMAND_LINE`. A second start sends its arguments to the running instance, waits until the instance did the command, and then stops. The commands `convert-shortcuts`, `--version` and `--help` run only in the new process.

The start at login uses an XDG autostart file with `Exec=petlauncher --hidden`. The program does not write that file yet.

Set the environment variable `PETLAUNCHER_DEBUG` to get more lines in the log: each show, hide, search, outcome and focus change. Each log line has the monotonic time in milliseconds.

## Launcher window

The launcher is an `AdwApplicationWindow` with the title `petlauncher`. It has no title bar. It has a query line and, below it, the rows of the results: an icon, a title and a subtitle.

| Subject | Behaviour |
|---|---|
| Width | The setting `window_width` (R5, initial value 750). A change of `settings.toml` has effect at once. |
| Height | 64 pixels with no rows. With rows, the window grows downward, up to 8 rows. Then the list scrolls. |
| Position | The GNOME Shell extension moves the window (R11). It puts the top of the window where a window of 64 pixels is at the center of the monitor. Thus the query line has the same place for each height. The value 64 is `COLLAPSED_HEIGHT` in `window.rs` and in the extension: change the two together. See [gnome-extension.md](gnome-extension.md). |
| Style | libadwaita follows the light or dark style of the system (R17). The program does not set a style, and its CSS uses only the colors of libadwaita. |
| Long text | A title gets an ellipsis at its end, and a subtitle in its middle. A long text does not make the window wider. |
| First show | The program prepares the window at its start (`realize`), so that the first show is fast. |

### Keys

| Key | Function |
|---|---|
| Enter, Ctrl+Enter, Shift+Enter | `activate` of the core on the selected row, or on the typed path when no row is selected (R1, R2, R13). |
| Down | Selects the next row. With no selected row (the contents of a typed directory, R1), Down selects the first row. |
| Up | Selects the row above. On the first row, Up removes the selection again when the results started with no selected row. Else the first row stays selected. The selection does not wrap at the ends of the list. |
| Escape | Hides the window. The query is kept (R9). |
| Tab | When Enter on the selected row changes the query, Tab does the same. Thus Tab completes a directory of the path mode. On other rows Tab does nothing. |
| All other keys | Go to the query line. |

A click on a row is Enter on that row.

When the owner presses Enter before the results of the text came, the key waits for these results. Thus Enter acts on the text that the owner typed, not on the rows of an older text.

### Outcome of a key

| Action | The program does |
|---|---|
| `Nothing` | Nothing. |
| `SetQuery` | Puts the text in the query line with the cursor at its end, and searches it. The window stays. |
| `Copy` | Puts the text on the clipboard. |
| `OpenDictionary` | Opens the dictionary window. Now it writes a log line only. |
| All other actions | `desktop::execute`, the seam for `petlauncher-gnome`. Now it writes a log line only. |

The window closes when the outcome says so.

### Show and hide

When the window shows, the program asks the core for the kept query (R9). With a kept query, the query line shows it and its rows, and all its text is selected. Thus a typed character replaces the query. With no kept query, the query line is empty and the window has its height of 64 pixels.

When the window hides, the program tells the core the reason:

| Reason | Cause |
|---|---|
| `Escape` | Escape, and also a close by the window manager, for example Alt+F4. |
| `FocusLoss` | A different window got the focus, and "Close petlauncher when losing focus" is on (R3). |
| `GlobalKey` | `petlauncher toggle` while the window shows. |
| `Activated` | The outcome of a key closed the window. |

The focus loss counts only after the window had the focus since its show. A window that shows has no focus until GNOME Shell activates it, so the program does not hide it at that time. The program reads the focus from the state `FOCUSED` of the GDK toplevel surface, not from the property `is-active` of the window: GTK 4.14 keeps `is-active` true after a hide, and then does not tell the focus of the next show.

The program selects the text of a kept query when the window gets the focus, not before. GTK claims the primary selection for selected text, and on Wayland that claim fails while the window has no keyboard focus. Then GTK removes the selection.

### Run in background

When "Run in background" is on, the process stays after the window hides. When it is off, the process stops 1 second after the window hides, if the window did not show again. On Wayland the text of a Copy belongs to the process. GNOME Shell keeps a copy of the clipboard when the process stops, but only after it read the text. A test showed the older clipboard text after a stop at once, and the copied text after a stop 1 second later.

## Actions on the session bus

The GNOME Shell extension uses these actions of the application. [gnome-extension.md](gnome-extension.md) gives the contract.

| Action | Function |
|---|---|
| `show` | Shows the launcher. When it shows already, it comes to the front, and the query does not change. |
| `settings` | Shows the settings window. Now this is a window with a short text and the title `petlauncher Settings`. |
| `quit` | Stops the program. |
| `tray-icon` | Disabled. Its boolean state is the setting `show_tray_icon`, and it changes when `settings.toml` changes. |

## Settings

The program reads `settings.toml` and `shortcuts.toml` with a `ConfigStore` and watches the directory with a `ConfigWatcher` ([settings.md](settings.md)). After a change of a file, the next search uses the new values. A new `window_width` and a new `show_tray_icon` have effect at once. The program reads the other settings when it needs them, for example `close_on_focus_loss` at a focus loss. When a file has an error, the old values stay and the program writes the error to the log.

## Threads

| Thread | Work |
|---|---|
| Main thread | GTK, the windows, the settings and the actions. |
| `petlauncher-search` | `Engine::search` (ADR 0002). For each change of the query, the main thread cancels the token of the last search and sends the new text. The thread searches only the newest text. It sends the results back to the main loop with an `async-channel`. The main thread shows them only when they are for the text in the query line now. |
| `petlauncher-config-watch` | The `ConfigWatcher`. It sends the name of a changed file to the main loop. |

## Seam for the next parts

The windows call the rest of the program only through `Backend` in `src/backend.rs` and `execute` in `src/desktop.rs`.

| File | Now | Next |
|---|---|---|
| `src/backend.rs` | `Backend` joins `Engine` (path mode, shortcuts, system commands), `ConfigStore`, `KeptQuery` and the search thread. Methods: `search`, `activate`, `window_opened`, `window_closed`, `execute`, `settings`, `reload`, `config_dir`. | The `Launcher` of `petlauncher-providers` gives the engine with all providers, the settings and the background work. |
| `src/desktop.rs` | `execute` writes the action to the log. | The `Desktop` of `petlauncher-gnome` executes the action. |
| `src/rows.rs` | `Icon::ForFile` gets the icon of the content type from the file name only. | No change. |
| `src/settings_window.rs` | A window with a short text. | The settings window. |
| `src/dictionary_window.rs` | A log line. | The dictionary window (R19). |
| none | The autostart file is not written. | Write and remove `~/.config/autostart/pet.repo.Petlauncher.desktop` with "Run in background" (R3). |

## Test

```sh
TMPDIR=$HOME/tmp cargo test -p petlauncher
cargo build -p petlauncher --release
gnome/extension/test/nested-test.sh --program target/release/petlauncher --out DIR
gnome/extension/test/nested-test.sh --program target/release/petlauncher --owner-layout --out DIR
```

The unit tests cover the modules with no widgets: the command line, the selection, the keys, the show and hide rules, the model of the window, the search thread and the backend. `tests/cli.rs` runs the binary for the commands that need no display.

`nested-test.sh --program` runs the real program with the real extension in a separate headless GNOME Shell, with two monitors. It sends keys with a virtual keyboard. `gnome/extension/test/program-steps.sh` has the steps, and `analyse-program.sh` makes the result tables. The test does not touch the session of the user.

### Results

Environment: GNOME Shell 46.0, GTK 4.14.5, libadwaita 1.5.0, Ubuntu 24.04, release build, on 2026-11-26. Raw evidence: `~/daybook-evidence/petlauncher/ui/` on the owner's computer, in `2026-11-26-program-logical-sizes` (3008x1692 and 1920x1200 at scale 1) and `2026-11-26-program-owner-layout` (scale 2 and 1.5).

- Position: in the two runs, all 17 shows of the launcher had the position of the rule on the monitor with the pointer, from the first painted frame. No monitor painted the window before the map, and each show had one position only.
- Query line: on each monitor, the top of the window was the same for the empty launcher (64 pixels high) and for a launcher that showed with a kept query and 8 rows (453 pixels high, 445 at scale 2).
- Keys: `~/pathtest/` and Enter gave `OpenFolder` of the directory. Down and Enter gave `OpenFile` of the newest file. Ctrl+Enter gave `ShowInFolder`, Shift+Enter gave `Copy` of the path, `lock` and Enter gave `System(Lock)`, `g rust` gave `OpenUrl` of the shortcut, and Tab on `~/pathtest/alp` completed the directory. A click on the first row gave `OpenFile`.
- Kept query: Escape and a focus loss kept the query. The next show had the text selected, and a typed character replaced it. An activation cleared the query.
- Tray icon: a click showed the launcher, the menu item "Settings" showed the settings window and did not move it, and "Quit" stopped the program. The icon followed `show_tray_icon` in `settings.toml`.
- Clipboard: the copied text stayed on the clipboard after the window hid, with "Run in background" on and off.
- Style: a change of the GNOME setting `color-scheme` changed the open window to dark and back. The test gives the display to the portal backends, so libadwaita reads the setting through the Settings portal, as in the session of the owner. In a trial with no display for the portal backends, the window did not change.
- Time from `petlauncher toggle` to the first buffer of the window in GNOME Shell, with the program running: 28 to 46 ms (40 to 64 ms in the owner layout). The first show after the start of the program took 230 to 250 ms. In these runs the program was the first GTK program of a new GNOME Shell. In a separate trial with 5 starts after that first one, the first show took 122 to 134 ms. A `petlauncher toggle` or `petlauncher` with no running program took 155 to 222 ms to the first buffer. The command `petlauncher toggle` itself took 23 to 44 ms, of which about 17 ms is the start of a process that loads GTK.

### Not tested

- The live session and the real monitors of the owner. The headless shell renders with no GPU, so the times on the owner's computer can be different.
- The desktop actions: the seam only writes them to the log.
- The global key of GNOME (`petlauncher toggle` was run by the test script).
- Input methods other than the keyboard layout `us`.
- A launcher that GNOME Shell does not activate.
