# The Launcher

The crate `petlauncher-providers` connects the search crates to the engine of `petlauncher-core`. It has one provider for each search crate, and the `Launcher`: the one object that the program holds. The `Launcher` owns the settings store, the engine with all providers, the kept query (R9), the usage counts of the applications and the background threads. The crate has no GTK, GLib, GNOME or D-Bus dependency.

## Use

```rust
let dirs = Dirs::from_env().expect("an absolute HOME");
let mut hooks = Hooks::new(petlauncher_apps::gnome::is_settings_page, move |update| {
    // On a background thread: send `update` to the main loop, which
    // searches the query again.
});
hooks.autostart = Autostart::from_env(program_path);
let launcher = Arc::new(Launcher::start(dirs, hooks)?);

// Worker thread (ADR 0002):
let results = launcher.search(&query, &cancel);

// Thread of the user interface:
let outcome = launcher.activate(&results, selected, Key::Enter);
// ... execute outcome.action ...
launcher.executed(&results.query, &outcome)?;
```

The rule for settings pages is knowledge of the desktop, so the desktop part gives it. The crate itself does not know GNOME.

## Surface

| Method | Thread | Function |
|---|---|---|
| `Launcher::start(dirs, hooks)` | Any | Reads the settings, the stored file index, the applications, the usage file and the stored rates. Starts the background work. About 10 ms in a release build. |
| `search(query, cancel)` | Worker | The `Results` of the engine. It never waits for the background work. |
| `activate(results, selected, key)` | Any | `petlauncher_core::activate`. |
| `executed(query, outcome)` | User interface | The program executed the outcome. A `LaunchDesktopEntry` adds one start to the usage counts. When `outcome.close` is true, this is also the close of the window for R9. |
| `window_opened()` | User interface | Gives the kept query, or `None`. Starts the background checks, see below. |
| `window_closed(query, reason)` | User interface | A close with no key on a row: loss of focus, Escape, global key. |
| `settings()`, `shortcuts()` | Any | The values in effect, as `Arc`. |
| `edit_settings(change)`, `edit_shortcuts(change)` | User interface | The editors of `ConfigStore`. The comments of the files stay. |
| `reload(file)` | Any | Reads a file again. The watcher calls it. |
| `config_error(file)`, `config_warnings()` | Any | R18: the error that keeps the last text of a file out of effect, and the warnings. |
| `file_search_error()` | Any | An ignore entry that the file index cannot use, for example a bad pattern. |
| `problems()` | Any | Problems of the background work, as text. |
| `dictionary_lookup(word)`, `dictionary_suggestions(text, limit)` | Any | For the dictionary window, through the same open dictionary. |
| `browsers()` | Any | The installed browsers for the settings window (R12). |
| `browser_start(browser, url)` | Any | How to open a URL in the browser of a shortcut. |
| `wait_idle()` | Any | Waits for the background work. For tests and the example. |

Do not call `window_closed` for a close that `executed` reported. If the program calls both, the second call decides the kept query, and R9 step 5 fails.

### Dirs and Hooks

`Dirs` gives all paths, so a test uses temporary directories:

| Field | `Dirs::from_env()` |
|---|---|
| `home` | `$HOME` |
| `config` | `$XDG_CONFIG_HOME/petlauncher`, else `~/.config/petlauncher` |
| `cache` | `$XDG_CACHE_HOME/petlauncher`, else `~/.cache/petlauncher` |
| `state` | `$XDG_STATE_HOME/petlauncher`, else `~/.local/state/petlauncher` |
| `applications` | `Environment::from_vars` with the variables of the process |
| `zoneinfo` | `$TZDIR`, else `/usr/share/zoneinfo` |
| `city_list` | `/usr/share/libgweather-4/Locations.xml` |

`Hooks::new(settings_page, on_change)` sets the values for the program. A test changes the fields:

| Field | `Hooks::new` | Test |
|---|---|---|
| `settings_page` | The argument | `gnome::is_settings_page` |
| `on_change` | The argument | A channel |
| `autostart` | `None` | A temporary config home |
| `watch_config` | `true` | `false`, the test calls `reload` |
| `clock` | `SystemClock` | `ManualClock` |
| `time_zone` | `TimeZone::system()` | `Europe/Helsinki` |
| `fetch_rates` | `fetch_ecb` with a timeout of 10 s | A function with a fixed answer |
| `outside_ignore_files` | `true` | `false`, so the test does not read the git settings of the owner |

`on_change` gets an `Update`: `Rates`, `Cities`, `Files`, `Applications`, or `Config(file)`. It runs on a background thread. After each update the program searches the query again. After `Config` it also shows the new settings and the errors.

## Providers and rows

| Slot | Provider | Rows |
|---|---|---|
| `Calculator` | `petlauncher-calc` | One `Calculation` row. The title is `Calculation::text()`. The subtitle is the note, for example `ECB reference rates of 2026-11-25`, else `Enter copies the result`. Enter copies the number. An error row (division by zero, units that do not convert, no rates) copies nothing. |
| `Time` | `petlauncher-time` | The rows of `Cities::answer`. Enter copies the main text. Before the city list is loaded there are no rows. |
| `Applications` | `petlauncher-apps` | At most 9 `Application` rows. Subtitle: the comment, else the generic name. Enter: `LaunchDesktopEntry`. The local file is the `.desktop` file (R2, R13). |
| `SettingsPages` | `petlauncher-apps` | At most 5 `SettingsPage` rows, with the subtitle `Settings`. They stay when "Include applications in search" is off (R3). |
| `FileSearch` | `petlauncher-index` | At most 50 `File` and `Directory` rows. The subtitle is the path with `~`. Enter opens a file, and puts the path of a directory with a last slash in the query line, as the path mode does. The core gives Ctrl+Enter and Shift+Enter from the local file (R1, R2, R13). |
| `Dictionary` | `petlauncher-dict` | See below. |

The applications and the settings pages come from one `Applications` list, which is searched one time with the usage counts. Thus a typing error shows only when no entry of the two kinds matches without one.

### Dictionary rows (R19)

For `d <text>`:

1. The meanings of the typed word and of its base forms, at most 20 rows. The title is the definition. The subtitle is the word, the word class and the synonyms, for example `launcher, noun. Synonyms: rocket launcher`.
2. The words that start with the text, without the typed word, at most 10 rows.
3. With no rows from steps 1 and 2: one row `"<text>" is not in the dictionary`, which does nothing.

Enter on a row of step 1 or 2 gives `OpenDictionary(word)`. When the WordNet directory is missing or incomplete, the only row is `Install the dictionary: sudo apt install wordnet-base`. Its subtitle has the path that was tried, and Enter copies the command.

The dictionary opens at the first `d` query or the first call of `dictionary_lookup` or `dictionary_suggestions`, not at the start. The open takes about 50 ms with the real data. A failed open stays until the window opens again or `wordnet_path` changes. `~` in `wordnet_path` is the home directory.

## Threads

| Thread | Work | End |
|---|---|---|
| Worker of the program | `search` | The program |
| `petlauncher-loader` | The city list at the start. A check of the application files after each `window_opened`. | Drop closes its channel and waits for it. |
| `petlauncher-rates` | The check of the rates at the start, after each `window_opened`, and each 6 hours. | Drop closes its channel. Drop waits for the thread, but not for a download that runs. That thread ends after the download, with no update. |
| Scan of the file index | A thread of `FileIndex` for each scan. | The scan stops when the index is dropped. |
| Settings watch | The thread of `ConfigWatcher`, when `watch_config` is true. | Drop stops the watch. |

No thread that answers a query downloads, scans or reads the application files.

## Background rules

| Data | At the start | When the window opens | Other |
|---|---|---|---|
| File index | Loads the stored index, then scans. | Scans when the last scan started 60 s or more ago, by `Hooks::clock`. | A change of the roots or the ignore list scans immediately (R21). Each end of a scan gives `Update::Files`. |
| Applications | Reads the list. | `has_changed()` on the loader thread, then a new list and `Update::Applications`. | |
| Currency rates | Loads the stored rates, then a check. | A check. | A check each 6 hours on the timer of the thread. |
| City list | Loads on the loader thread, then `Update::Cities`. | | |
| Dictionary | | Forgets a failed open. | Opens at the first use. |

A check of the rates fetches only when the last check is 6 hours or more ago and the date of the rates in effect is before the current date in the owner's time zone. The check at the start always counts as due. The program sends a check at each window open because the timer of the thread does not count the time in the suspend state. The wall clock does.

A fetch gives the text of the ECB file. When it is a valid file, its rates go into effect, the file goes to the cache directory and `Update::Rates` follows. Rates with an earlier date than the rates in effect are ignored. A failed fetch keeps the old rates and is not a problem for the owner: the calculation rows show the date of the rates. A cache file that cannot be written is a problem in `problems()`.

Limit: when a fetch fails, the next fetch comes after 6 hours. With no stored rates, a currency query gives `No currency rates available` until then.

## Settings

A search takes a copy of the settings and the shortcuts, so a reload does not wait for a search. After `edit_settings`, and after a `reload` of `settings.toml` that changed the values, the `Launcher` gives the new values to the parts that do not read them at each search:

- the scope of the file index, with `FileIndex::set_scope`, which scans immediately (R21 step 3),
- the WordNet directory,
- the autostart file.

The keywords of the file search and the dictionary need nothing, because each search reads them.

A file with an error keeps the old values in effect, and `config_error` gives the error with the line (R18). An ignore entry that the index cannot use, for example the pattern `[`, keeps the old scope of the index, and `file_search_error` gives the error. At the start, such an entry gives the initial scope of `FileSearch::default()`.

With `watch_config`, the `Launcher` starts a `ConfigWatcher` on the settings directory. After each report it calls `reload`. A result other than `Unchanged` gives `Update::Config(file)`. A write by the settings window gives `Unchanged`.

## Start at login (R3)

The module `autostart` writes `autostart/pet.repo.Petlauncher.desktop` below the config home when "Run in background" is on, and removes it when the setting is off. `Autostart { config_home, command }` gives the directory and the `Exec` value. The program gives its own path as the command. The `Launcher` applies the setting at the start and after each change of the settings. With `Hooks::autostart` `None`, it does not touch the file.

The file is written only when its text is different. Thus a new program path takes effect at the next start, and a change that the owner made in the file is lost at that time. A command with a line end is an error.

## Browsers (R12)

`browsers()` gives the desktop entries of the installed web browsers in the order of their names. The ID of an entry is the value for `browser = { app = "..." }`.

`browser_start(browser, url)` gives:

| Browser | Result |
|---|---|
| `Browser::App(id)` | `BrowserStart::DesktopEntry { path, command }`: the `.desktop` file for `gio::DesktopAppInfo`, and the `Exec` arguments with the URL. `None` when the entry is not installed. |
| `Browser::Command(text)` | `BrowserStart::Command(["sh", "-c", "<text> \"$1\"", "sh", url])`. The shell reads the command, and the URL is its last argument. |

## Tests and example

The tests make a temporary home below `TMPDIR` with fixture `.desktop` files, a generated WordNet directory with invented meanings, small city files, a rates file, a file tree and a fixed clock at 2026-11-26 18:04 in Helsinki. The fetch of the rates is a function with a fixed answer. No test reads the real home directory, the real settings, the real WordNet data or the network.

The example program starts a `Launcher` on the real directories of the user, with no settings watch and no autostart file, and prints the rows for each query:

```sh
cargo run --release -p petlauncher-providers --example query -- lock "100 usd in eur" "d launcher"
```

It reads the settings and does not write them. It can write the cache files: the file index and the rates. It downloads the rates when the stored rates are old.
