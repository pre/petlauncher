# The settings window

The settings window shows and changes the settings of [settings.md](settings.md) and the global key of GNOME (R4). The process has one settings window with the title `petlauncher Settings`. These open it:

- the item "Settings" of the tray menu (the action `settings`),
- Ctrl+comma in the launcher, and the gear button at the right edge of the query line. The launcher hides, as with Escape, and the settings window comes to the front with the page that it showed last.
- the button "Open Global Key Settings" of the R4 notice in the launcher. The launcher hides, and the window shows the Global Key page (the action `settings-page` with the parameter `'global-key'`). The window does not open the key dialog by itself.

The window is an `AdwApplicationWindow` and follows the light or dark style of the system. A list at the left edge shows the pages with their icons and full titles, and the selected page shows at the right (`AdwNavigationSplitView`). Decision: a list, not the view switcher of `AdwPreferencesWindow`, because the view switcher at the top cut the titles, for example "Gene...", "Glob...". The window has no search field, which `AdwPreferencesWindow` had. The form of a shortcut is a subpage above the list and the pages (`AdwNavigationView`). A close hides the window. When "Run in background" is off, the process stops 1 second after the close if no window of the program shows (`stop.rs`).

The window is not modal. A modal window gets the keys of the launcher: with the modal `AdwPreferencesWindow` of an earlier version, Escape in the launcher closed the settings window.

## Pages

| Page | Contents |
|---|---|
| General | The four switches of R3. The window width in pixels, from 300 to 7680 (R5). The time of the kept query (R9). The settings files, with a button that opens each file in the default text editor and a button that opens the settings folder. |
| Global Key | One row with the key that GNOME has for petlauncher. Its button opens the dialog that records a new key (R4). |
| Search | The file search: its keyword, the directories and the ignore list (R14, R21). The dictionary: its keyword, the folder of the WordNet data and the state of the data (R19). |
| Shortcuts | The list of the shortcuts with add, change, remove and move (R6, R7, R12). A shortcut opens in a form. |

### Kept query (R9)

"Keep the query" has four choices: for a time in seconds, for a time in minutes, forever, and no (the query is cleared at each close). With a time, the row "Time" gives the number, 30 seconds at the start. A time of whole minutes shows in minutes, for example `keep_query = "120s"` shows 2 minutes.

When "Run in background" is off, the two rows are disabled and the window shows the warning of `petlauncher_core::keep_query_warning`: the process stops when its window closes, so no query is kept.

### Global key (R4)

The window reads the key from GNOME each time it shows the page, so the window and GNOME Settings show the same key. The row "Show and hide petlauncher" has a button with the key, for example `Super+Space`, or "Set shortcut…" when petlauncher has no key.

The button opens the dialog "Set the Global Key" (`key_dialog.rs`). The dialog starts with the key of GNOME, or with the setting `hotkey` when GNOME has no key for petlauncher. Thus at the first start with a conflict, the dialog shows Super+Space and its conflicts at once. In the dialog:

- A key combination with its modifiers is the new key. A modifier key alone waits for the next key. `validate_accelerator` checks the key, and the dialog shows its error.
- BackSpace with no modifier offers no key. Set then removes the key of petlauncher.
- Escape with no modifier closes the dialog with no change.

GNOME Shell takes its own keys, for example Super+Space, before a window gets them. While the dialog shows, it asks GNOME Shell to pause its shortcuts (`gdk::Toplevel::inhibit_system_shortcuts`), and stops the pause when it closes. GNOME Shell asks the owner first: "Allow inhibiting shortcuts". The dialog shows the state: "GNOME Shell pauses its shortcuts, so all keys come to this dialog", or that GNOME Shell did not pause them and keeps its keys. Thus the owner does not type a key as text.

When other key bindings of GNOME Settings have the new key, the dialog shows each of them with its name, its section in GNOME Settings and a button "Remove". The button removes only that key from that binding (`CustomShortcuts::remove_key`): a custom shortcut keeps its entry with an empty key, and a binding of GNOME keeps its other keys. The shortcut of petlauncher itself is never in the list. Then the list shows the bindings that still have the key.

"Set" is active only for a correct key that is not the key now and that no other binding has, and for no key when petlauncher has a key. Decision: an explicit "Set", not a write after the last "Remove", so that the owner sees the result of the removals before petlauncher takes the key. "Set" writes the key with `set_global_key_if_free`, or an empty key for no key: the shortcut "Show petlauncher" then stays with an empty key. Then the program:

- writes the key to the setting `hotkey` in `settings.toml`, `""` for no key,
- writes the marker file of the first installation, so the program does not write the key of `hotkey` again,
- removes the notice of R4 in the launcher.

### File search and dictionary (R14, R19, R21)

A keyword or a path is saved with Enter or the button at the end of its row. A directory or an ignore entry is added with Enter or the button, or selected with the folder button. A folder below the home directory is written as `~/...`. The trash button of a row removes that entry.

An ignore entry is a path such as `~/tmp`, a directory name such as `node_modules`, or a pattern such as `*-wt`. `IgnoreEntry::parse` checks the entry before the save, and the window shows the error of a bad pattern below the entry. The typed text stays, so the owner can correct it. When `settings.toml` has a bad pattern from an editor, the window shows `Launcher::file_search_error` below the list.

The `Launcher` applies the settings after each change, so a change of the file search updates the index at once.

The dictionary shows "The WordNet data is found", or the cause, the folder that petlauncher tried, and the install command with a button that copies it.

### Shortcuts (R6, R7, R12)

Each row shows the name, the keyword and the first line of the command. A shortcut with a warning or an error has an icon with the text in its tooltip. The button `+` opens an empty form, and a click on a row opens the form of that shortcut.

To move a shortcut, drag its row by the handle at the right edge of the row. A line above or below the row under the pointer shows the new place: the upper half of a row puts the shortcut above it, the lower half below it. A click on the handle does not open the form. Alt+Up and Alt+Down move the focused row one place, and the row keeps the focus. Each move is `ShortcutEdit::Move`, so the comment lines above a shortcut move with it. `ShortcutEdit::drop` and `ShortcutEdit::step` give the move with no widgets. A list that the window cannot change has no handles.

The form has the fields of a shortcut: name, keyword, icon (an icon name or the path of an image, with a preview), the command (a URL or a script, in a field with more lines), the switch "Run without argument" and below it the field "Different command without argument (optional)" (R7), "Default search", and the browser (R12): the default browser, an installed browser of `Launcher::browsers` by its name, or a command. A browser of the shortcut that is not installed shows as "ID (not installed)", so a save keeps it.

The field of the command without argument shows only when the switch "Run without argument" is on. With the switch on and an empty field, Enter with only the keyword runs the command with an empty argument. The file and the core do not change: the core also runs a stored `cmd_without_argument` when `run_without_argument` is false. Thus `ShortcutForm`:

- turns the switch on when the file has `run_without_argument = true` or a `cmd_without_argument`,
- saves `run_without_argument` with the value of the switch, and `cmd_without_argument` only when the switch is on, else `""`,
- keeps the typed text while the switch is off, so a switch off and on again does not lose it,
- checks "The command is empty" with the values that the save writes.

The form checks the values with the rules of `petlauncher_config` on the list after the save, and shows each problem below its field:

- An error stops the save: an empty keyword, a keyword with a space, an empty browser command.
- A warning does not stop it: a keyword of a different shortcut, the keyword of the file search or the dictionary, an empty name, an empty command.

A new form shows its problems only after the first change, so an empty form does not start with warnings. "Remove Shortcut" asks first, then removes the shortcut and the comment lines directly above it.

## Changes in an editor and errors (R18)

Each change of the window writes only its value through the editors of the store (`SettingsEditor`, `ShortcutsEditor`), so the comments and the layout of the files stay. The module `edits` maps each change to the editor call.

When the owner saves a file in an editor while the window shows, the program gets `Update::Config` and the window shows the new values.

When a file has an error, the page shows it at the top, with the file, the line and the column, and a button that opens the file. The old values stay in effect, and the rows of that file are disabled, so the window does not write the file until the error is gone. A shortcut form that is open when the error comes keeps its values: its save shows the refusal of the store, and the form stays open.

`EditError::FileChanged`: `shortcuts.toml` changed on the disk after the last read, so the numbers of the shortcuts can be different. The store reads the file again, and the window shows the new list. An open form finds its shortcut again by the values that it had when the form opened (`ShortcutForm::retarget`), and keeps the typed values. When that shortcut is gone, the form saves a new shortcut. The form shows "shortcuts.toml changed in an editor ... Do the change again", and the owner selects Save again. The form also finds its shortcut again after each `Update::Config`, so a save never changes a different shortcut.

`EditError::InlineShortcuts`: the file has the shortcuts as an inline list, `shortcut = [{ ... }]`. The window cannot change it and keep its comments. The page shows the shortcuts with the cause and a button that opens the file, and it has no buttons that change the list. The window finds this with an edit that changes nothing, so it writes nothing.

## Parts of the program

| File | Contents |
|---|---|
| `src/settings_window/mod.rs` | `SettingsWindow` with the list of pages, `Page` (the pages and their names for the action `settings-page`), the trait `Host` (what the window needs from the program) and the shared `Context` with the saves. |
| `src/settings_window/edits.rs` | `SettingsEdit` and `ShortcutEdit`: each change and its editor call, the move of a drop and of Alt+Up or Alt+Down, the check of an ignore entry, and the texts of a refused change. |
| `src/settings_window/form.rs` | `ShortcutForm`: the values of one shortcut, its problems at the fields, its save, and `retarget`. |
| `src/settings_window/hotkey.rs` | `HotkeyState`: the key of GNOME, the new key or no key, its error, the conflicts and the state of Set. `capture`: what a key press does in the key dialog. |
| `src/settings_window/keep_time.rs` | `KeepTime`: the number and the unit of R9 and `KeepQuery`. |
| `*_page.rs`, `form_page.rs`, `key_dialog.rs`, `widgets.rs` | The widgets. |

The plain modules have unit tests. The tests of `edits` use a `Launcher` on temporary directories and compare the files. The tests of `hotkey` use the memory GSettings backend with the test schemas of `petlauncher-gnome`.

## Test

```sh
TMPDIR=$HOME/tmp cargo test -p petlauncher
cargo build -p petlauncher --release
gnome/extension/test/nested-test.sh --program target/release/petlauncher \
    --steps gnome/extension/test/settings-steps.sh --out DIR
```

`settings-steps.sh` runs in the headless GNOME Shell of [program.md](program.md), with the R4 conflict on Super+Space at the start. It finds a widget by its role and name through AT-SPI (`a11y.py`, with the system Python), then clicks it with the virtual pointer or types with the virtual keyboard of the shell. A drag uses the button of the virtual pointer (`petProbe.button`). When GNOME Shell asks to allow the pause of its shortcuts, the steps select Allow with Return (`petProbe.modalCount` tells that the question shows). Each check writes a line to `acceptance.tsv`.

### Results

Environment: GNOME Shell 46.0, GTK 4.14.5, libadwaita 1.5.0, Ubuntu 24.04, release build, monitors 3008x1692 and 1920x1200 at scale 1, on 2026-11-26. The evidence directory of the run is not in the repository. It has `acceptance.tsv`, the AT-SPI trees (`a11y-*.txt`), the key files and the settings files of the steps, the program log and the screenshots.

- 89 checks, 89 pass: the button of the R4 notice (the Global Key page shows), the gear, Ctrl+comma; R4 (the pause of the GNOME Shell shortcuts, Super+Space in the dialog, the two conflicts, Remove on each, Set, the key file, the marker file, the notice in the launcher, a pressed key, Escape, BackSpace and Set, a key again); R9 step 8; R3 (the four switches with their effect, and the autostart file); R5; R21 steps 1 to 4, a bad pattern and Remove; R19 (the state of the data and a missing folder); R12; R7 steps 1 and 2 with the switch "Run without argument", and the checks at the fields; R6 (a drag, a click on the handle, Alt+Up, and Remove); R18 (a change in an editor, an error and its correction); an inline list and an error in `shortcuts.toml`; the stop rule.
- R4: before the first write, the GSettings backend was the key file of the test. GNOME Shell asked "Allow inhibiting shortcuts" (`shell-inhibit-question.png`), and after Allow the log had `shortcuts of GNOME Shell paused true` and `pressed "<Super>space"`, while the input source switch of GNOME Shell still had Super+Space. After the two Remove buttons, the custom shortcut of the invented program had its entry with `binding=''`, and the input source switch had `['XF86Keyboard']`.
- Ctrl+comma: the frame of the settings window was the same at its map and after it, so the extension did not move it (ADR 0005). `summary.txt` has the table.
- R12 and R7: `launches.log` has `edge | https://docs.example.com/?q=cats`, `browser | https://github.com/pre/rektangel/pull/123` and `browser | https://github.com/pre/rektangel/pulls`.
- R21: the test file had no `[file_search]` table. The diff of `settings.toml` (`settings-r21.diff`) adds the table with the initial ignore list and the new entry, and changes no other line.
- Screenshots of each page in the light and the dark style: `settings-general-light.png`, `settings-global-key-dark.png` and the others. Each state of the steps has a screenshot, for example `launcher-notice.png`, `settings-from-notice.png`, `settings-r4-page.png`, `settings-r4-conflicts.png`, `settings-r4-no-key.png`, `settings-shortcuts-drag.png`, `settings-form-switch-off.png`, `settings-form-switch-on.png`, `settings-r9-background-off.png`, `settings-form-error-dark.png`, `settings-r18-error.png` and `settings-shortcuts-inline.png`.

Changes after a look at the screenshots of the first run: clearer texts of the kept-query choices; the key as GNOME Settings shows it (Super+Space) in the texts of the key page; texts of the keyword problems for the form, with no shortcut number; no warnings in an empty new form; no empty line of a hidden message group; the icon preview at the end of its row; the bookmarks icon for the Shortcuts page. A test found that the window was modal (see the start of this file). After this run: the image of a dragged row has the background of the row, because the list draws the background of its rows; space above the field of the command without argument.

### Not tested

- The folder button (the file chooser of the portal), the buttons that open a file or the settings folder, and the copy button of the install command.
- `EditError::FileChanged` in the window: the race between a change on the disk and the watch. The unit tests cover the store and `ShortcutForm::retarget`.
- A shortcut form that is open when its file gets an error.
- In the window: the time of the kept query with a number and a unit, the directories of the file search, the keywords of the file search and the dictionary, and a browser command in a shortcut. The unit tests cover their edits.
- The global key of GNOME itself: no gnome-settings-daemon runs in the test shell, and GNOME Settings did not show the key. The test checked the key file.
- The answer Deny to the question of GNOME Shell about the pause of its shortcuts. The dialog then shows that GNOME Shell did not pause them.
- A drag with a touch screen.
- The owner layout with the scales 2 and 1.5, the live session, and a screen reader.
