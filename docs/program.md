# The program

The crate `crates/petlauncher` is the program `petlauncher`: the command line, the application on the session bus and the launcher window. It uses GTK 4 and libadwaita. It joins the `Launcher` of `petlauncher-providers` ([launcher.md](launcher.md)) and the GNOME desktop of `petlauncher-gnome` ([gnome-desktop.md](gnome-desktop.md)). This file describes the command line, the window, the global key, the threads, the build and the test. [install.md](install.md) describes the installation.

The settings window: [settings-window.md](settings-window.md). The dictionary window: [dictionary-window.md](dictionary-window.md).

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
| `petlauncher --hidden` | Starts the program with no window. This is the command of the autostart file. When the program runs already, nothing changes. When "Run in background" is off, the program stops at once. |
| `petlauncher toggle` | Shows the launcher, or hides it when it shows. When the program does not run, this process starts it and shows the launcher. The global key runs this command (R4). |
| `petlauncher --version` | Prints the version. |
| `petlauncher --help` | Prints the commands. |

The exit status is 0 on success and 2 for a wrong argument.

One instance of the program runs in each session. It is a `GApplication` with the ID `pet.repo.Petlauncher` and the flag `HANDLES_COMMAND_LINE`. A second start sends its arguments to the running instance, waits until the instance did the command, and then stops. The commands `--version` and `--help` run only in the new process.

| Environment variable | Effect |
|---|---|
| `PETLAUNCHER_DEBUG` | More lines in the log: each show, hide, search, row list, outcome, execution and focus change. Each log line has the monotonic time in milliseconds. |
| `PETLAUNCHER_OFFLINE` | No download of the currency rates. The calculator uses the stored rates in `~/.cache/petlauncher/ecb-eurofxref-daily.xml`. The headless test sets it, because the test has no network. |

## Launcher window

The launcher is an `AdwApplicationWindow` with the title `petlauncher`. It has no title bar. It has a query line with a gear button at its right edge, below it a line for each notice, and then the rows of the results: an icon, a title and a subtitle. The gear does the same as Ctrl+comma. It takes no focus, so the typed keys stay in the query line.

| Subject | Behaviour |
|---|---|
| Width | The setting `window_width` (R5, initial value 750). A change of `settings.toml` has effect at once. |
| Height | 64 pixels with no rows and no notice. With rows, the window grows downward, up to 8 rows. Then the list scrolls. |
| Move | A press and a drag with the mouse on the free space of the query line or of the notices moves the window: the search icon, the padding and the text of a notice (`GtkWindowHandle`). The entry, the buttons and the rows keep their clicks. The launcher keeps the focus during the move. |
| Pin | After a move by the owner, a pin button shows beside the gear, with the tooltip "Open Here from Now On". It writes the place to `custom_position` and `window_position = "custom"` ([settings.md](settings.md)), then goes away until the next move. It takes no focus, and the launcher stays. The extension gives the place (ADR 0009). |
| Position | The GNOME Shell extension moves the window (R11, ADR 0007) to the place of the setting `window_position`. A launcher that the owner moved opens at that place again while the query is kept (ADR 0009). It puts the top of the window where a window of 64 pixels is at the center of the monitor. Thus the query line has the same place for each height. The value 64 is `COLLAPSED_HEIGHT` in `window.rs` and in the extension: change the two together. See [gnome-extension.md](gnome-extension.md). |
| Style | libadwaita follows the light or dark style of the system (R17). The program does not set a style, and its CSS uses only the colors of libadwaita. |
| Long text | A title gets an ellipsis at its end, and a subtitle in its middle. A long text does not make the window wider. |
| First show | The program prepares the window at its start (`realize`), so that the first show is fast. |

### Keys

| Key | Function |
|---|---|
| Enter, Ctrl+Enter, Shift+Enter | `activate` on the selected row, or on the typed path when no row is selected (R1, R2, R13). |
| Down | Selects the next row. With no selected row (the contents of a typed directory, R1), Down selects the first row. |
| Up | Selects the row above. On the first row, Up removes the selection again when the results started with no selected row. Else the first row stays selected. The selection does not wrap at the ends of the list. |
| Escape | Hides the window. The query is kept (R9). |
| Ctrl+comma | Hides the window as Escape does, and shows the settings window. |
| Tab | When Enter on the selected row changes the query, Tab does the same. Thus Tab completes a directory of the path mode. On other rows Tab does nothing. |
| All other keys | Go to the query line. |

A click on a row is Enter on that row.

When the owner presses Enter before the results of the text came, the key waits for these results. Thus Enter acts on the text that the owner typed, not on the rows of an older text.

When the `Launcher` reports an update, for example the end of a scan of the file index, new applications or a changed settings file, the program searches the query again. The selected row stays selected when it is still in the new rows, and the list keeps its scroll position.

### Outcome of a key

| Action | The program does |
|---|---|
| `Nothing` | Nothing. |
| `SetQuery` | Puts the text in the query line with the cursor at its end, and searches it. The window stays. |
| `Copy` | Puts the text on the clipboard. |
| `OpenDictionary` | Opens the dictionary window with the word (`dictionary_window::open`, [dictionary-window.md](dictionary-window.md)). |
| All other actions | `GnomeDesktop::execute`: open a file, a folder or a URL, start an application, run a script, or a system command. |

The program executes the action while the launcher window is still visible, and hides the window after it, so that the new window gets the activation token of the window with the focus. When the folder opens with the default application because the file manager interface failed, the log gets the reason.

When an action fails, the window stays open and shows the error in a line below the query line (`DesktopError`, for example "The browser petfixture-missing.desktop is not installed."). The query and the rows stay, so the owner can select a different row. A new text in the query line and the next show remove the error.

After a successful action the program calls `Launcher::executed`. It records the start of an application for the usage counts, and the close of the window for the kept query (R9).

### Notices

The line below the query line shows these notices, the most urgent first:

| Notice | When it goes |
|---|---|
| The error of the last action | A new text in the query line, or the next show. |
| R4: the global key is not set, because a different key binding has it. The notice gives the key, for example Super+Space, and the section and name of each binding with it. Its button "Open Global Key Settings" hides the launcher and opens the settings window on the Global Key page. The button takes no focus. | The owner sets or removes the key in the settings window, or the next show after the owner removed the key in GNOME Settings. |
| R18: a settings file has an error, so its old values stay in effect. The notice gives the file, the line and the error. | The file is correct again. |

### Show and hide

When the window shows, the program asks the `Launcher` for the kept query (R9). With a kept query, the query line shows it and its rows, and all its text is selected. Thus a typed character replaces the query. With no kept query, the query line is empty.

When the window hides, the kept query gets one record of the close:

| Reason | Cause | Record |
|---|---|---|
| `Escape` | Escape, and also a close by the window manager, for example Alt+F4. | `window_closed` |
| `FocusLoss` | A different window got the focus, and "Close petlauncher when losing focus" is on (R3). | `window_closed` |
| `GlobalKey` | `petlauncher toggle` while the window shows. | `window_closed` |
| `Activated` | The outcome of a key closed the window. | `executed` only. A second record would decide the kept query again, and R9 step 5 would fail. |

The focus loss counts only after the window had the focus since its show. A window that shows has no focus until GNOME Shell activates it, so the program does not hide it at that time. The program reads the focus from the state `FOCUSED` of the GDK toplevel surface, not from the property `is-active` of the window: GTK 4.14 keeps `is-active` true after a hide, and then does not tell the focus of the next show.

The program selects the text of a kept query when the window gets the focus, not before. GTK claims the primary selection for selected text, and on Wayland that claim fails while the window has no keyboard focus. Then GTK removes the selection.

### Run in background

When "Run in background" is on, the process stays after the window hides, and the `Launcher` writes the autostart file `~/.config/autostart/pet.repo.Petlauncher.desktop` with the path of this program and `--hidden` (R3). When the setting is off, the `Launcher` removes the file, and the process stops 1 second after a window of the program hides, if no window of the program shows then (`stop.rs`). The launcher, the dictionary window and the settings window each keep the process. On Wayland the text of a Copy belongs to the process. GNOME Shell keeps a copy of the clipboard when the process stops, but only after it read the text. A test showed the older clipboard text after a stop at once, and the copied text after a stop 1 second later.

## Global key (R4)

At the start, and at each show until it is done, the program does the first installation of the global key:

1. When the marker file `~/.local/state/petlauncher/global-key-installed` exists, nothing happens.
2. When GNOME has a key for the petlauncher shortcut already, or the setting `hotkey` is empty, the program writes the marker file and changes nothing.
3. Else it calls `CustomShortcuts::set_global_key_if_free(hotkey, "petlauncher toggle")`. With no conflict, GNOME gets the custom shortcut "Show petlauncher", and the program writes the marker file. With a conflict, the program writes nothing and no marker, and the launcher shows the notice of R4.

The program never changes a different key binding. Only the key dialog of the settings window removes the key from a conflicting binding, with `CustomShortcuts::remove_key`, after an explicit action of the owner ([settings-window.md](settings-window.md)). When the owner sets or removes the key in the settings window, the program also writes the marker file and removes the notice. While the marker file exists, the program does not write the key again, so a key that the owner removed in GNOME Settings stays removed. Delete the file to do the first installation again.

## Actions on the session bus

The GNOME Shell extension uses the actions `show`, `settings`, `quit`, `tray-icon`, `window-position`, `kept-query-until` and `launcher-moved` of the application. [gnome-extension.md](gnome-extension.md) gives the contract. The launcher also uses `settings-page`.

| Action | Function |
|---|---|
| `show` | Shows the launcher. When it shows already, it comes to the front, and the query does not change. |
| `settings` | Shows the settings window ([settings-window.md](settings-window.md)). |
| `settings-page` | Parameter: the name of a page, `'general'`, `'global-key'`, `'search'` or `'shortcuts'`. Shows the settings window with that page. With a different name, the window shows the page that it showed last. |
| `quit` | Stops the program. |
| `tray-icon` | `GnomeDesktop::add_actions` adds it. Disabled. Its boolean state is the setting `show_tray_icon`, and `set_tray_icon_visible` changes it after each change of `settings.toml`. |
| `window-position` | `GnomeDesktop::add_actions` adds it. Disabled. Its `(sdd)` state is the setting `window_position` and the place of `custom_position` (ADR 0009). `set_window_position` changes it after each change of `settings.toml`. |
| `launcher-moved` | Parameter `(dd)`: the place of a move by the owner, as fractions of the work area. The launcher shows its pin button (ADR 0009). |
| `kept-query-until` | `GnomeDesktop::add_actions` adds it. Disabled. Its int64 state tells until when a show has the kept query (ADR 0009). `set_kept_query_until` changes it after each hide and each change of `settings.toml`. |

## Threads

| Thread | Work |
|---|---|
| Main thread | GTK, the windows, the actions and the calls of `GnomeDesktop`. |
| `petlauncher-search` | `Launcher::search` (ADR 0002). For each change of the query, the main thread cancels the token of the last search and sends the new text. The thread searches only the newest text. It sends the results back to the main loop with an `async-channel`. The main thread shows them only when they are for the text in the query line now. |
| The threads of the `Launcher` | The settings watch, the loader, the rates and the scans of the file index ([launcher.md](launcher.md)). `Hooks::on_change` sends each `Update` through an `async-channel` to the main loop. |

## Parts of the program

| File | Contents |
|---|---|
| `src/backend.rs` | `Backend`: the `Launcher` with the hooks of the program, the search thread, and `GnomeDesktop`. The windows call only its methods: `search`, `activate`, `execute`, `executed`, `window_opened`, `window_closed`, `settings`, `config_notice`, `problems`, `desktop`, `dictionary`, `config_dir`, `state_dir`, and `launcher` for the settings window. |
| `src/desktop.rs` | `execute`: an action with `GnomeDesktop::execute`. |
| `src/global_key.rs` | `install_once`: the first installation of the global key (R4). `record_owner_choice`: the marker file after a change in the settings window. |
| `src/notices.rs`, `src/model.rs`, `src/selection.rs`, `src/keys.rs`, `src/visibility.rs`, `src/worker.rs` | The rules of the window, with no widgets. |
| `src/window.rs`, `src/rows.rs`, `src/app.rs` | The widgets and the application. |
| `src/stop.rs` | "Run in background" off: the stop when no window of the program shows. |
| `src/settings_window/` | The settings window ([settings-window.md](settings-window.md)). |
| `src/dictionary_window/` | The dictionary window (R19). Its `DictionarySource` is the `Launcher`, so the launcher rows and the window use one open dictionary. |

## Test

```sh
TMPDIR=$HOME/tmp cargo test -p petlauncher
cargo build -p petlauncher --release
gnome/extension/test/nested-test.sh --program target/release/petlauncher --out DIR
gnome/extension/test/nested-test.sh --program target/release/petlauncher --owner-layout --out DIR
```

The unit tests cover the modules with no widgets: the command line, the selection, the keys, the show and hide rules, the notices, the model of the window, the search thread, the backend with a `Launcher` on temporary directories, and the first installation of the global key with the memory GSettings backend and the test schemas of `petlauncher-gnome`. `tests/cli.rs` runs the binary for the commands that need no display.

`nested-test.sh --program` runs the real program with the real extension and the real `GnomeDesktop` in a separate headless GNOME Shell, with two monitors. The test does not touch the session of the user:

- It has its own session bus, a fake system bus, its own `HOME` and XDG directories, and the key file GSettings backend. Before the first start of the program, the test checks that the default GSettings backend of its environment is the key file of the test.
- `mock-services.js` owns `org.freedesktop.FileManager1`, `org.gnome.SessionManager` and `org.gnome.ScreenSaver` on the session bus, and `org.freedesktop.login1` on the fake system bus. It records each call, and nothing real happens.
- The fixture applications, the default applications of `mimeapps.list` and a settings page are recorders that write their arguments to a file.
- The stored currency rates are a fixture, and `PETLAUNCHER_OFFLINE` stops the download.
- The key file starts with a custom shortcut of an invented program on Super+Space, and the input source switch has its GNOME default, which is also Super+Space.
- A virtual keyboard of the shell types the queries, and the clipboard of the shell gives the pasted text.

`acceptance-steps.sh` has the acceptance steps of the requirements. Each check writes a line to `acceptance.tsv`, and `analyse-program.sh` prints the table.

### Results

Environment: GNOME Shell 46.0, GTK 4.14.5, libadwaita 1.5.0, Ubuntu 24.04, release build, on 2026-11-26. There were two runs: one with the monitors 3008x1692 and 1920x1200 at scale 1, and one with `--owner-layout` (scale 2 and 1.5). Their evidence directories are not in the repository. Each has `summary.txt` with all tables, `acceptance.tsv`, `mock-calls.jsonl`, `launches.log`, the key files of the steps of R4, the program log and the screenshots.

- Acceptance: 103 checks in each run, 103 pass. `acceptance.tsv` has R1 steps 1 to 9, R2 steps 1 and 2, R3 (the applications off, and "Run in background" off with the dictionary window and the settings window), R4 (a conflict, the key after the owner removed the conflicts, no new key after the owner removed it), R7 steps 1 and 2, R8 steps 1 and 2, R9 steps 1 to 7, R10 steps 1 to 8, R12 (an installed browser, a browser command with no shell, and a browser that is not installed), R13, R14, R15 (a currency with the stored rates), R16 (the five system commands and a settings page), R18 (a settings file with an error), R19 steps 1 to 3 and R20 (the three forms).
- Keep time of R9: the test uses `keep_query = "3s"` and waits 3.5 s for "after the keep time", not 30 s.
- System commands (R16): `Lock()`, `Suspend(true)` on the fake system bus, `Logout(0)`, `Reboot()` and `Shutdown()` reached the mocks. `ShowFolders` and `ShowItems` reached the mock of the file manager with a real activation token of the test shell as `StartupId`.
- Global key (R4): the first start wrote nothing and showed the notice with the two conflicts (a custom shortcut and the input source switch of GNOME). After the owner removed the two keys, the next show wrote "Show petlauncher" with `<Super>space` and the marker file. After the owner removed the key of petlauncher, the next start did not write it again. Before the first start, the test checked that the GSettings backend was the key file of the test.
- Position: in the two runs, all 18 shows of the launcher had the position of the rule from the first painted frame, also the first show with the notice of R4 (171 pixels high, 165 at scale 2). The query line had the same place on each monitor for the empty launcher and for a launcher with 8 rows (453 pixels high, 445 at scale 2).
- "Run in background" off: the dictionary window and the settings window still showed 3 s after the launcher hid, and the process stopped after each window closed. The copied text stayed on the clipboard after the stop.
- Time from `petlauncher toggle` to the first buffer of the window in GNOME Shell, with the program running: 21 to 37 ms (25 to 36 ms in the owner layout). The first show after the start of the program, which was the first GTK program of a new GNOME Shell: 194 ms (235 ms). A start of the program that shows the launcher: 172 to 178 ms (173 and 580 ms). The command `petlauncher toggle` itself: 18 to 33 ms.

After the change of the example names, `2026-11-26-full-after-rename` (3008x1692 and 1920x1200 at scale 1) ran the same steps: 103 checks, 103 pass, all shows of the launcher at the place of the rule, and 20 to 34 ms from `petlauncher toggle` to the first buffer of a show.

`settings-steps.sh` tests the settings window: [settings-window.md](settings-window.md).

`dictionary-steps.sh` (through `nested-test.sh --steps`, with `PETLAUNCHER_WORDNET_PATH`) ran with the real dictionary rows of the `Launcher`, in `2026-11-26-dictionary-launcher-rows`. The window opened from `d launcher`, its links, history and search worked, it stayed when the launcher showed and hid again, and the extension did not move it on either monitor. The first page took 0.05 ms for the lookup, because the `Launcher` had opened the dictionary for the launcher rows. With missing data the launcher gives only the row "Install the dictionary", so the step reached the page of the missing data through the search field of the window. With "Run in background" off the process ran 3 s after the launcher hid and stopped after the window closed.

Screenshots that the owner can examine: `launcher-first-m1.png` (the notice of R4), `application-rows.png` (icons and a long comment), `calculation-row.png`, `currency-row.png`, `time-row.png`, `dictionary-rows.png`, `dictionary-window.png`, `error-notice.png` (a browser that is not installed), `settings-error-notice.png`, `down-selected-m0.png` and `down-selected-m0-dark.png` (light and dark).

### Not tested

- The live session and the real monitors of the owner. The headless shell renders with no GPU, so the times on the owner's computer can be different.
- The real file manager, the real session manager, the screen saver and logind: the test has mock services, so the real confirmation windows of GNOME, the selection in Files and the lock screen were not seen. `docs/gnome-desktop.md` has the check list for the live session.
- The global key of GNOME: no gnome-settings-daemon runs in the test shell, so the test checked the custom shortcut in the key file, and `petlauncher toggle` was run by the test script.
- The download of the currency rates: `PETLAUNCHER_OFFLINE` stopped it.
- The focus of a new window that petlauncher starts: the fixture applications have no window.
- Input methods other than the keyboard layout `us`.
- A launcher that GNOME Shell does not activate.
- The scripts `install.sh` and `uninstall.sh` with the real prefix `~/.local`: they ran only with a prefix below `$HOME/tmp`.
