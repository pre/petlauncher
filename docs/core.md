# The core: contract, modes and path mode

The crate `petlauncher-core` holds the behaviour of petlauncher that does not depend on the desktop or on GTK. This file describes the contract with the user interface, how the engine selects the mode of a query, the rules of the path mode, and how the user interface must call the engine.

The core does not start a process, open a URL, use the clipboard or call D-Bus. It gives `Action` values. The program executes them, and gives the functions that depend on the desktop to the `Desktop` trait.

## Contract

All public data types are `Send`, `Clone` and have no lifetimes, so the user interface can keep them and move them between threads.

| Type | Purpose |
|---|---|
| `Row` | One row: `kind`, `title`, `subtitle`, `icon`, `local_file` and the `enter` action. |
| `RowKind` | `Calculation`, `Time`, `Application`, `SystemCommand`, `SettingsPage`, `Shortcut`, `File`, `Directory`, `Dictionary`. |
| `Icon` | `None`, a theme icon `Name`, an image `Path`, or `ForFile`: the icon of the content type of a file, which the user interface gets from GIO. |
| `Action` | What the program does. See the table below. |
| `Results` | The `query` text, the `mode`, the `rows`, the `selected` row at the start, and the `typed_path` of the path mode. |
| `Key` | `Enter`, `CtrlEnter`, `ShiftEnter`. |
| `Outcome` | The `action`, `close` (close the window) and `keep_query` (R9). |

| Action | The program does | `Desktop` method |
|---|---|---|
| `Nothing` | Nothing. The window stays open. | |
| `OpenFile(path)` | Opens the file with its default application. | |
| `OpenFolder(path)` | Opens the directory in the file manager. | `open_folder` |
| `ShowInFolder(path)` | Opens the folder of the item in the file manager, with the item selected. | `show_in_folder` |
| `SetQuery(text)` | Puts the text in the query line and searches again. The window stays open. | |
| `Copy(text)` | Puts the text on the clipboard. | |
| `LaunchDesktopEntry(path)` | Starts the `.desktop` file. Applications and settings pages use it. | |
| `OpenUrl { url, browser }` | Opens the URL. `browser` is `None` for the default browser (R12). | |
| `RunScript { script, argument }` | Runs the script with `sh`, with the argument as `$1`. `%s` is already replaced. | |
| `System(command)` | Lock, suspend, log out, restart or power off. | `run_system_command` |
| `OpenDictionary(word)` | Opens the dictionary window (R19). | |

All actions except `Nothing` and `SetQuery` close the window.

### Keys on a row

`activate(&results, selected, key)` gives the `Outcome`. `selected` is the row that the user interface selects now. It can differ from `results.selected` after the arrow keys.

| Key | Row with a local file | Row with no local file |
|---|---|---|
| Enter | `row.enter` | `row.enter` |
| Ctrl+Enter | `Directory` row: `OpenFolder(local_file)`. Other rows: `ShowInFolder(local_file)`. | `Nothing` |
| Shift+Enter | `Copy` of the full path of `local_file` | `Nothing` |

With no selected row, the keys act on the typed path of the path mode:

| Key | Typed directory | Typed file | No typed path |
|---|---|---|---|
| Enter | `OpenFolder` | `OpenFile` | `Nothing` |
| Ctrl+Enter | `OpenFolder` | `ShowInFolder` | `Nothing` |
| Shift+Enter | `Copy` of the path | `Copy` of the path | `Nothing` |

`keep_query` is true only for Enter on a `Calculation` row that closes the window. Ctrl+Enter, Shift+Enter and Enter on all other rows clear the query (R9).

The local file of an application row and of a settings page row is its `.desktop` file (R2, R13). A row of a URL shortcut, a calculation or a dictionary word has no local file.

## How the user interface calls the engine

`Engine::search(query, settings, shortcuts, cancel)` reads the file system and calls the providers. Thus the user interface must not call it on the GTK main thread.

1. Make one `Engine` at the start, register the providers, and put it in an `Arc`.
2. Start one worker thread. For each change of the query text, the user interface cancels the `CancelToken` of the last search, makes a new token, and sends the query, the token and the current settings and shortcuts to the worker.
3. The worker takes the newest query from its channel, drops the older ones, and calls `search`.
4. The worker sends the `Results` back to the main thread, for example with `glib::MainContext::channel` or `async-channel` and `glib::spawn_future_local`.
5. The main thread shows the results only when `results.query` is the text in the query line now. A cancelled search can give incomplete rows. Do not show them.

The engine reads the token before each provider, and the path mode reads it for each directory entry. A provider that can be slow must read it in its loops.

The settings and the shortcuts are arguments of each search. When `settings.toml` or `shortcuts.toml` change (R18), the next search uses the new values, and the engine needs no lock.

## Providers

A provider gives rows for a text:

```rust
pub trait Provider: Send + Sync {
    fn search(&self, text: &str, settings: &Settings, cancel: &CancelToken) -> Vec<Row>;
}
```

A closure with the same arguments is also a provider. To add a provider, register it in a slot:

```rust
let mut engine = Engine::new(home);
engine
    .register(Slot::Calculator, CalcProvider::new())
    .register(Slot::Applications, AppsProvider::new(entries))
    .register(Slot::SettingsPages, SettingsPages::new(pages))
    .register(Slot::FileSearch, IndexProvider::new(index));
```

| Slot | Rows | Text |
|---|---|---|
| `Calculator` | R8, R15 | The full query |
| `Time` | R20 | The full query |
| `Applications` | R3 setting "Include applications in search" off: not asked | The full query |
| `SystemCommands` | R16. `Engine::new` adds `SystemCommands`. | The full query |
| `SettingsPages` | R16 | The full query |
| `FileSearch` | R14 | The argument after the file search keyword |
| `Dictionary` | R19 | The argument after the dictionary keyword |

The providers of one slot come in the order of registration. Each provider sorts its own rows.

A provider of the general search (`Calculator` to `SettingsPages`) must answer from memory in a few milliseconds, because the engine asks it for each typed character. A provider that can be slow, such as the file index or the dictionary, must be in a keyword slot. No provider can wait for the network. For example, the currency rates of R15 come from the cache, and a background task updates them.

`match_rank` is the text match of the built-in rows. The comparison ignores letter case and all characters that are not letters or digits. A start of the title is the best match, then a start of a word of the title, then a start of a keyword.

## Mode of a query

The engine examines the query in this order. The first rule that applies selects the mode.

1. **Empty**: the query has only spaces. No rows.
2. **Path**: the query is a path. See "Path mode".
3. **File search**: the first word is `settings.file_search.keyword` (initial value `f`), and a space follows it. The rows of `Slot::FileSearch` for the rest of the query.
4. **Dictionary**: the first word is `settings.dictionary.keyword` (initial value `d`), and a space follows it. The rows of `Slot::Dictionary`.
5. **Shortcut**: the first word is the keyword of one or more shortcuts, and a space follows it. One row for each of these shortcuts, with the rest of the query as the argument.
6. **General**: all other queries.

The comparison of a keyword ignores letter case. The argument has no spaces at its start and end. The engine removes the spaces at the end of the query before rule 3, so `rektangel ` is the same as `rektangel`.

The file search keyword and the dictionary keyword win over a shortcut with the same keyword. For this reason the Cambridge Dictionary shortcut moves from `d` to `dd` (R6).

A keyword with no argument is a general search. Thus `f` and `d` alone show the general rows, and the rows of a file search or a dictionary come only after the space.

### General search

The rows come in this order:

1. The shortcuts whose keyword is the full query, with no argument. Thus `rektangel` gives the `rektangel` row first (R7 step 2).
2. `Slot::Calculator`, then `Slot::Time`.
3. The rows of `Slot::Applications`, `Slot::SystemCommands` and `Slot::SettingsPages` whose title matches the query with `match_rank`: the title or a word of the title starts with the query. The applications are not asked when "Include applications in search" is off (R3).
4. The other rows of the same three slots. They match only in other text, for example inside a word or in a keyword.
5. When steps 1 to 4 give no rows: one row for each shortcut with `is_default_search`, with the full query as the argument (R6).

In steps 3 and 4 the rows keep the order of the slots, and in each slot the order of the provider. Reason: the search of the applications also finds a text inside a word, so `lock` finds Clocks. Without step 4, Clocks comes before the system command Lock, and Enter starts Clocks.

The first row is selected at the start.

## Shortcut rows

| Case | Enter |
|---|---|
| An argument | `cmd` with `%s` replaced by the argument |
| No argument, and `cmd_without_argument` is not empty | `cmd_without_argument` (R7) |
| No argument, and `run_without_argument` is on | `cmd` with `%s` replaced by an empty text |
| No argument, in all other cases | `Nothing`. The subtitle tells the owner to type an argument (R6). |

A command that starts with `http://` or `https://` is a URL, and gives `OpenUrl` with the `browser` of the shortcut (R12). In a URL, the argument is percent-encoded: all bytes except the letters, the digits and `-._~/` become `%XX`. Thus a space, `&` or `#` in a search text does not change the URL.

All other commands are scripts, and give `RunScript`. In a script, `%s` becomes the argument with no change, and the argument is also `$1`. Use `"$1"` in a script when the argument can contain shell characters.

The `icon` of a shortcut is a file when it starts with `/` or `~/`, else a name in the icon theme.

## Path mode

### When the query is a path

The engine changes the query in this order:

1. It removes the spaces, tabs and line ends at the start and at the end.
2. It removes one pair of `"` or `'` around the text.
3. It removes `file://` or `file://localhost` at the start, and decodes the `%XX` codes.

The result is a path when it starts with `/` or `~/`, or is `~`. `~` is the home directory that the program gives to `Engine::new`. `~user` is not a path. The engine removes `.` parts and the last slash, so `~/a/b/` and `~/a/b` are the same path.

### Three cases

| Typed path | Rows | Selected at the start | `typed_path` |
|---|---|---|---|
| An existing directory, with or without the last slash | The contents of the directory | None | `Directory` |
| An existing file | One row for that file | The row | `File` |
| Not an existing path | The matches in the last folder of the path that exists | The first row, when there are rows | None |

A symbolic link counts as its target. The engine does not show a link with no target.

### Order of the contents of a directory

The newest modification time comes first. Entries with the same time come in name order, with no letter case. Thus after Down, Enter opens the newest file. Ulauncher puts the last access first. petlauncher uses the modification time, because the access time depends on the mount options (`relatime`, `noatime`) and changes when a program only reads a file.

Hidden entries (the name starts with `.`) do not show.

### Match of a path that does not exist

The engine walks the parts of the path from the root. The last part that is an existing folder is the folder of the search. The next part is the typed name. The parts after it have no effect. For example, in `~/lolcat-outptu/L0L3-IMAGE/` the folder is `~` and the typed name is `lolcat-outptu`.

1. The names that contain the typed name, with no letter case, come first. The same name comes before a name that starts with the typed name, and that comes before a name that contains it in another position. In each group the names come in name order.
2. When no name contains the typed name, the near matches show: the names that start with the same 3 or more characters as the typed name. The longest shared start comes first. Thus `palette-zoom.html` shows `palette-zoom.png` first (R10 note).

Hidden entries show only when the typed name starts with `.`.

### Rows of the path mode

| Row | `title` | `subtitle` | Enter |
|---|---|---|---|
| File | The file name | The path | `OpenFile` |
| Directory | The directory name | The path | `SetQuery` with the path of the directory and a last slash |

The icon is `Icon::ForFile` and the local file is the path. When the query starts with `~`, the subtitle and the new query use `~` for the home directory. Else they use the full path.

The path mode shows at most 50 rows.

## Kept query (R9)

`KeptQuery` is a state machine with an injected `Clock`. The program calls `window_closed(query, reason, settings)` when the window closes, and `window_opened(settings)` when it opens.

| `CloseReason` | Keeps the query |
|---|---|
| `FocusLoss`, `Escape`, `GlobalKey` | Yes |
| `Activated { keep_query }` | The `keep_query` of the `Outcome` |

The query is not kept when "Run in background" is off, when `keep_query` is `disabled`, or when the query is empty. `window_opened` gives the query when it is still kept, and then forgets it. The value of `keep_query` at the time of the open decides: `forever` keeps it, a number of seconds keeps it for less than that time after the close, and `disabled` gives `None`.

The clock is the wall clock (`SystemTime`), so the time in the suspend state also counts. When the wall clock goes back, the elapsed time is zero.

The user interface puts the query in the query line, selects all its text and searches it again. Then a typed character replaces the query and an arrow key moves in it.

`keep_query_warning(settings)` gives the warning for the settings window when "Run in background" is off.

## Desktop trait

The program calls the `Desktop` trait. The core does not call it. `petlauncher-gnome` implements it, see [gnome-desktop.md](gnome-desktop.md).

| Method | Requirement |
|---|---|
| `open_folder`, `show_in_folder` | R1, R2. The result `FolderOpened` tells if the file manager opened the folder, or if the desktop part used the default application for folders as a fallback. |
| `run_system_command` | R16 |
| `global_key`, `set_global_key` | R4 |
| `center_window` | R11 |
| `set_tray_icon_visible` | R3 |

The methods run on the thread of the user interface, so the trait does not need `Send`.

These functions are not in the trait:

- The settings pages (R16). `petlauncher-apps` finds them with a rule that the desktop part gives, `petlauncher_gnome::is_settings_page` on GNOME. The `pages` in the example above come from these entries.
- The light or dark style (R17). libadwaita follows the style of the system by itself: the program does not set a color scheme, and reads `AdwStyleManager` when it must know the style.
