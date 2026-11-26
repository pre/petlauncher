# The settings window

The settings window shows and changes the settings of [settings.md](settings.md) and the global key of GNOME (R4). The process has one settings window with the title `petlauncher Settings`. These open it:

- the item "Settings" of the tray menu (the action `settings`),
- Ctrl+comma in the launcher. The launcher hides, as with Escape, and the settings window comes to the front.

The window is a libadwaita preferences window and follows the light or dark style of the system. A close hides it. When "Run in background" is off, the process stops 1 second after the close if no window of the program shows (`stop.rs`).

The window is not modal. The template of `AdwPreferencesWindow` makes it modal, and GTK then gives the keys of the launcher to the modal window: Escape in the launcher closed the settings window, and the typed text went into the search field of the settings window. The program sets `modal` to false.

## Pages

| Page | Contents |
|---|---|
| General | The four switches of R3. The window width in pixels, from 300 to 7680 (R5). The time of the kept query (R9). The settings files, with a button that opens each file in the default text editor and a button that opens the settings folder. |
| Global Key | The key that GNOME has for petlauncher, a new key, and the other key bindings with the new key (R4). |
| Search | The file search: its keyword, the directories and the ignore list (R14, R21). The dictionary: its keyword, the folder of the WordNet data and the state of the data (R19). |
| Shortcuts | The list of the shortcuts with add, change, remove and move (R6, R7, R12). A shortcut opens in a form. |

### Kept query (R9)

"Keep the query" has four choices: for a time in seconds, for a time in minutes, forever, and no (the query is cleared at each close). With a time, the row "Time" gives the number, 30 seconds at the start. A time of whole minutes shows in minutes, for example `keep_query = "120s"` shows 2 minutes.

When "Run in background" is off, the two rows are disabled and the window shows the warning of `petlauncher_core::keep_query_warning`: the process stops when its window closes, so no query is kept.

### Global key (R4)

The window reads the key from GNOME each time it opens, so the window and GNOME Settings show the same key. "Key now" shows the key, for example `Super+Space    <Super>space`, or "No key".

To set a new key, type it in "New key", for example `<Control><Super>k`, or select "Press a Key" and press the key with its modifiers. Escape stops the wait. GNOME Shell keeps some keys for itself, for example the key of a key binding, and such a key does not reach the window: type it. `validate_accelerator` checks the text, and the window shows its error.

When other key bindings of GNOME Settings have the new key, the window shows each of them with its name, its place in GNOME Settings and a button "Remove". The button removes only that key from that binding (`CustomShortcuts::remove_key`): a custom shortcut keeps its entry with an empty key, and a binding of GNOME keeps its other keys. The shortcut of petlauncher itself is never in the list. Then the list shows the bindings that still have the key.

"Set" shows only when the new key is correct, is not the key now, and no other binding has it. Decision: an explicit "Set", not a write after the last "Remove", so that the owner sees the result of the removals before petlauncher takes the key. "Set" writes the key with `set_global_key_if_free`. Then the program:

- writes the key to the setting `hotkey` in `settings.toml`,
- writes the marker file of the first installation, so the program does not write the key of `hotkey` again,
- removes the notice of R4 in the launcher.

"Remove Key" gives petlauncher no global key: the shortcut "Show petlauncher" stays with an empty key, `hotkey` becomes `""`, and the marker file is written.

### File search and dictionary (R14, R19, R21)

A keyword or a path is saved with Enter or the button at the end of its row. A directory or an ignore entry is added with Enter or the button, or selected with the folder button. A folder below the home directory is written as `~/...`. The trash button of a row removes that entry.

An ignore entry is a path such as `~/tmp`, a directory name such as `node_modules`, or a pattern such as `*-wt`. `IgnoreEntry::parse` checks the entry before the save, and the window shows the error of a bad pattern below the entry. The typed text stays, so the owner can correct it. When `settings.toml` has a bad pattern from an editor, the window shows `Launcher::file_search_error` below the list.

The `Launcher` applies the settings after each change, so a change of the file search updates the index at once.

The dictionary shows "The WordNet data is found", or the cause, the folder that petlauncher tried, and the install command with a button that copies it.

### Shortcuts (R6, R7, R12)

Each row shows the name, the keyword and the first line of the command. The arrow buttons move a shortcut up or down. A shortcut with a warning or an error has an icon with the text in its tooltip. The button `+` opens an empty form.

The form has the fields of a shortcut: name, keyword, icon (an icon name or the path of an image, with a preview), the command (a URL or a script, in a field with more lines), the command without argument (R7), "Default search", "Run without argument", and the browser (R12): the default browser, an installed browser of `Launcher::browsers` by its name, or a command. A browser of the shortcut that is not installed shows as "ID (not installed)", so a save keeps it.

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
| `src/settings_window/mod.rs` | `SettingsWindow`, the trait `Host` (what the window needs from the program) and the shared `Context` with the saves. |
| `src/settings_window/edits.rs` | `SettingsEdit` and `ShortcutEdit`: each change and its editor call, the check of an ignore entry, and the texts of a refused change. |
| `src/settings_window/form.rs` | `ShortcutForm`: the values of one shortcut, its problems at the fields, its save, and `retarget`. |
| `src/settings_window/hotkey.rs` | `HotkeyState`: the key of GNOME, the new key, its error, the conflicts and the active buttons. `capture`: the accelerator of a key press. |
| `src/settings_window/keep_time.rs` | `KeepTime`: the number and the unit of R9 and `KeepQuery`. |
| `*_page.rs`, `form_page.rs`, `widgets.rs` | The widgets. |

The plain modules have unit tests. The tests of `edits` use a `Launcher` on temporary directories and compare the files. The tests of `hotkey` use the memory GSettings backend with the test schemas of `petlauncher-gnome`.

## Test

```sh
TMPDIR=$HOME/tmp cargo test -p petlauncher
cargo build -p petlauncher --release
gnome/extension/test/nested-test.sh --program target/release/petlauncher \
    --steps gnome/extension/test/settings-steps.sh --out DIR
```

`settings-steps.sh` runs in the headless GNOME Shell of [program.md](program.md), with the R4 conflict on Super+Space at the start. It finds a widget by its role and name through AT-SPI (`a11y.py`, with the system Python), then clicks it with the virtual pointer or types with the virtual keyboard of the shell. GTK 4.14 gives a wrong x to the page tabs of the header bar, so `a11y.py tab` puts the row of tabs at the center of the window. Each check writes a line to `acceptance.tsv`.

### Results

Environment: GNOME Shell 46.0, GTK 4.14.5, libadwaita 1.5.0, Ubuntu 24.04, release build, monitors 3008x1692 and 1920x1200 at scale 1, on 2026-11-26. The evidence directory of the run is not in the repository. It has `acceptance.tsv`, the AT-SPI trees (`a11y-*.txt`), the key files and the settings files of the steps, the program log and the screenshots.

- 76 checks, 76 pass: Ctrl+comma; R4 (the two conflicts, Remove on each, Set, the key file, the marker file, the notice in the launcher, a captured key, a wrong text, Remove Key, a typed key); R9 step 8; R3 (the four switches with their effect, and the autostart file); R5; R21 steps 1 to 4, a bad pattern and Remove; R19 (the state of the data and a missing folder); R12; R7 steps 1 and 2 with a shortcut of the window, and the checks at the fields; R6 (move and remove); R18 (a change in an editor, an error and its correction); an inline list and an error in `shortcuts.toml`; the stop rule.
- R4: before the first write, the GSettings backend was the key file of the test. After the two Remove buttons, the custom shortcut of the invented program had its entry with `binding=''`, and the input source switch had `['XF86Keyboard']`.
- Ctrl+comma: the frame of the settings window was the same at its map and after it, so the extension did not move it (ADR 0005). `summary.txt` has the table.
- R12 and R7: `launches.log` has `edge | https://docs.example.com/?q=cats`, `browser | https://github.com/pre/rektangel/pull/123` and `browser | https://github.com/pre/rektangel/pulls`.
- R21: the test file had no `[file_search]` table. The diff of `settings.toml` (`settings-r21.diff`) adds the table with the initial ignore list and the new entry, and changes no other line.
- Screenshots of each page in the light and the dark style: `settings-general-light.png`, `settings-global-key-dark.png` and the others. Each state of the steps has a screenshot, for example `settings-r4-conflicts.png`, `settings-r9-background-off.png`, `settings-form-warning.png`, `settings-form-error-dark.png`, `settings-r18-error.png` and `settings-shortcuts-inline.png`.

Changes after a look at the screenshots: clearer texts of the kept-query choices; the key as GNOME Settings shows it (Super+Space) in the texts of the key page; texts of the keyword problems for the form, with no shortcut number; no warnings in an empty new form; no empty line of a hidden message group; the icon preview at the end of its row; the bookmarks icon for the Shortcuts page. A test found that the window was modal (see the start of this file).

### Not tested

- The folder button (the file chooser of the portal), the buttons that open a file or the settings folder, and the copy button of the install command.
- `EditError::FileChanged` in the window: the race between a change on the disk and the watch. The unit tests cover the store and `ShortcutForm::retarget`.
- A shortcut form that is open when its file gets an error.
- In the window: the time of the kept query with a number and a unit, the directories of the file search, the keywords of the file search and the dictionary, and a browser command in a shortcut. The unit tests cover their edits.
- The global key of GNOME itself: no gnome-settings-daemon runs in the test shell, and GNOME Settings did not show the key. The test checked the key file.
- A key that GNOME Shell keeps for itself, in "Press a Key".
- The owner layout with the scales 2 and 1.5, the live session, and a screen reader.
