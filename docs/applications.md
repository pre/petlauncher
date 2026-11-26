# Applications and settings pages

The crate `petlauncher-apps` reads the desktop entries (`.desktop` files) of the installed applications and of the settings pages, and searches them. It has no GTK, GLib, GNOME or D-Bus dependency. It does not start a process. This file gives the directory rule, the filter rule, the settings page rule, the match and rank rule, the start data, the browser list, the change check and the usage file.

## Use

```rust
let env = Environment::from_process();
let apps = Applications::load(env, gnome::is_settings_page);
let mut usage = UsageStore::open(&cache_dir)?;

let hits = apps.search("te", 9, usage.counts());
let first = hits[0].entry;         // first.kind: Application or SettingsPage
                                   // first.path: the .desktop file (R2, R13)
usage.record_start(&first.id)?;    // after Enter starts it

if apps.has_changed() {
    let fresh = apps.reload();     // can run on a different thread
}
```

`Applications` is `Send` and `Sync`. `reload` gives a new list and does not change the old list, so a background thread can make the new list while the search uses the old one.

## Directories

1. The data directories are `$XDG_DATA_HOME`, then each directory of `$XDG_DATA_DIRS` in its order.
2. When `XDG_DATA_HOME` is missing, empty or not an absolute path, the value is `$HOME/.local/share`.
3. When `XDG_DATA_DIRS` is missing or empty, the value is `/usr/local/share:/usr/share`. A relative directory and a repeated directory are ignored.
4. The loader reads the `applications` directory of each data directory, and all its subdirectories. It follows symbolic links, for example the links that flatpak makes. A directory is read one time only, also when a link points to it again.
5. Only files with names that end in `.desktop` are entries.
6. The desktop file ID is the path below `applications`, with `-` in place of each `/`. Example: `applications/kde4/kate.desktop` has the ID `kde4-kate.desktop`.
7. The first file with an ID owns the ID. A file with the same ID in a later directory has no effect. This is also true when the first file has `Hidden=true` or cannot be read. GLib does the same.

`Environment::from_vars(home, var)` takes the home directory and the variables as parameters, so a test does not read the real environment. `Environment::from_process()` reads the variables of the process.

On this desktop on 2026-11-26, `XDG_DATA_HOME` is not set and `XDG_DATA_DIRS` has 7 directories. These data directories have an `applications` directory:

| Directory | `.desktop` files |
|---|---|
| `~/.local/share/applications` | 6 |
| `/usr/share/ubuntu/applications` | 3 |
| `~/.local/share/flatpak/exports/share/applications` | 1, a symbolic link |
| `/usr/share/applications` | 221 |
| `/var/lib/snapd/desktop/applications` | 9 |

## Keys

The loader reads only the group `[Desktop Entry]`. It reads `Type`, `Name`, `GenericName`, `Comment`, `Keywords`, `Icon`, `Exec`, `TryExec`, `Categories`, `MimeType`, `OnlyShowIn`, `NotShowIn`, `StartupWMClass`, `Terminal`, `NoDisplay` and `Hidden`. `DesktopEntry` keeps the full path of the file.

| Subject | Rule |
|---|---|
| Localized keys | `Name`, `GenericName`, `Comment`, `Keywords` and `Icon`. For the locale `lang_COUNTRY.ENCODING@MODIFIER`, the loader tries `Name[lang_COUNTRY@MODIFIER]`, `Name[lang_COUNTRY]`, `Name[lang@MODIFIER]`, `Name[lang]` and then `Name`. The encoding has no effect. |
| Locale | The first value that is not empty of `LC_ALL`, `LC_MESSAGES` and `LANG`. `C` and `POSIX` have no translations. `LANGUAGE` is not read. |
| Escapes | `\s`, `\n`, `\t`, `\r` and `\\`. Other escape sequences stay in the value, so that the `Exec` quoting can read them. |
| Lists | Split at `;`. `\;` is a `;` in an item. Empty items are removed. |
| Booleans | `true` and `1` are true. All other values are false. |

Ubuntu puts the translations of some entries in gettext files (`X-Ubuntu-Gettext-Domain`, 82 files on this desktop). The loader does not read gettext files, so these entries have the name in English.

Parser: the crate has its own parser, about 190 lines for the key file and the locale. The crate `freedesktop-desktop-entry` 0.8.4 has 16 crates in its dependency tree with no default features, and its default feature builds the C library gettext. It also reads the environment by itself. The crate `freedesktop_entry_parser` 2.0.1 (`nom` and `indexmap`) reads only the key file format, so the locale rule, the escapes and the lists stay to do. The only dependency is `unicode-normalization` (with `tinyvec`), for the accent removal of the search.

## Filter rule

An entry is in the search when all these conditions are true:

1. `Type=Application`.
2. `Hidden` is not true.
3. `NoDisplay` is not true, or the entry is a settings page.
4. The desktop names of `XDG_CURRENT_DESKTOP` permit it: for each name in the order of the variable, a name in `OnlyShowIn` shows the entry, and a name in `NotShowIn` hides it. When no name is in a list, the entry shows only if it has no `OnlyShowIn`. The names are compared with case.
5. When `TryExec` has a value, that program is an executable file: an absolute path, or a name in a directory of `PATH`.
6. The first argument of `Exec` is an executable file in the same way.

Decision for `TryExec` and `Exec`: the loader removes the entry when the program is missing, as GLib does. Reason: `gio::DesktopAppInfo` cannot start such an entry, and the GNOME application grid does not show it. On this desktop, 5 entries have a missing `Exec` program, for example `texdoctk.desktop`.

`errors()` gives each file that the loader could not use: a file that cannot be read, a file with no group `[Desktop Entry]`, an application with no `Name`, and an `Exec` with a quote that does not end.

Result on this desktop on 2026-11-26: 57 applications and 27 settings pages, with no errors. The 57 desktop file IDs are the same as the IDs for which `Gio.AppInfo.should_show()` is true (system `python3` with GObject introspection, GLib of Ubuntu 24.04).

## Settings page rule

The kind of an entry is `EntryKind::Application` or `EntryKind::SettingsPage`. The caller gives the rule for settings pages to `Applications::load` as a `SettingsPageRule`, which is `fn(&DesktopEntry) -> bool`. The rule for GNOME is `gnome::is_settings_page`. A different desktop, for example KDE, gives its own rule, and the rest of the crate does not change.

`gnome::is_settings_page`: the `Exec` starts the program `gnome-control-center` with the name of a page, for example `gnome-control-center wifi`. Rules:

- A leading `env` and its `NAME=value` arguments are skipped.
- The program can have a directory, for example `/usr/bin/gnome-control-center`.
- At least one more argument does not start with `-`. Thus `gnome-control-center` alone, which is the application "Settings", is an application.

Rules that were examined on this desktop (GNOME Settings 46.7):

| Rule | Files | Result |
|---|---|---|
| `Exec` starts `gnome-control-center` with a page | 27 | Selected. It is what the entry does: it opens GNOME Settings on that page. |
| The file name ends in `-panel.desktop` | 27 | Not selected. It is only a convention of the file names. |
| `Categories` has `X-GNOME-Settings-Panel` | 29 | Not selected. It also selects "Language Support" and "Software & Updates", which are separate programs. |
| The key `X-GNOME-Settings-Panel` | 2 | Not selected. Only the two separate programs have it. |

The settings pages also obey the filter rule. For example `gnome-ubuntu-panel.desktop` has `OnlyShowIn=ubuntu;`, so it shows only when `XDG_CURRENT_DESKTOP` has the name `ubuntu`.

## Match and rank

The search compares normalized text. To normalize, the crate changes letters to small letters, removes accents (canonical decomposition, then removal of the combining marks), and keeps only letters and digits. A word starts after a character that is not a letter or digit, and at a capital letter after a small letter. Example: `Wi-Fi` becomes `wifi` with the words `wi` and `fi`, and `LibreOffice` has the words `libre` and `office`. The query is normalized in the same way, so `wifi`, `Wi-Fi` and `WI FI` are the same query.

The search compares the localized name, the generic name, the keywords, and the program name. The program name is the file name of the first `Exec` argument after `env`. The program name is not used for a wrapper (`bash`, `env`, `flatpak`, `gjs`, `pkexec`, `python`, `python2`, `python3`, `sh`, `wine`, `wine64`, the list of GLib), and not for a settings page, because all settings pages have the program `gnome-control-center`.

Each entry gets the best tier that matches:

| Tier | Points | Example query, entry |
|---|---|---|
| The name is the query | 1000 | `wifi`, Wi-Fi |
| The name starts with the query | 900 | `te`, Terminal |
| A word of the name starts with the query | 850 | `code`, Visual Studio Code. Also `studio co`. |
| Each query word starts a word of the name, in any order | 820 | `vis code`, Visual Studio Code |
| The initials of the name start with the query (two or more characters) | 800 | `gc`, Google Chrome |
| The program name starts with the query | 750 | `nautilus`, Files |
| A word of a keyword starts with the query | 700 | `shell`, Terminal |
| A word of the generic name starts with the query | 650 | `brow`, Google Chrome (Web Browser) |
| A word of the name contains the query (three or more characters) | 500 | `ditor`, Text Editor |
| A word of a keyword or of the generic name contains the query (three or more characters) | 400 | `rowser`, Google Chrome |
| One typing error in the start of a word of the name (four or more characters) | 200 | `termnial`, Terminal |

Rules for the tiers:

- A text inside a word matches only inside one word. Thus `set` does not find "Mouse & Touchpad".
- A typing error is one substitution, insertion, deletion, or exchange of two adjacent characters. The first character must be correct. Typing error results show only when no entry matches without an error. Thus `disp` gives Displays and not Disks.
- An empty query, or a query with no letters or digits, gives no results.

Usage: the caller gives the number of starts of each desktop file ID. The score is the points of the tier plus 25 points for each doubling of the starts, at most 200: `floor(25 × log2(1 + starts))`. 1 start gives 25, 3 starts give 50, 20 starts give 109, and 255 starts give 200.

Effect of the usage points:

- In a tier, the entry with more starts goes first. With 3 starts of Text Editor, `te` gives Text Editor before Terminal.
- A frequent entry can pass the entries of the next better tiers. With 20 starts, Google Chrome (850 + 109) passes Characters (900) for `ch`.
- A text inside a word gets at most 500 + 200 = 700 points, so it never passes a match of the name, the initials or the program name (750 or more).

Results with equal scores come in this order: the shorter normalized name first, then the normalized name in alphabetical order, then the desktop file ID.

Results on this desktop with no usage, on 2026-11-26:

| Query | First results |
|---|---|
| `te` | Terminal, Text Editor, Microsoft Teams (PWA), Vim, kukka |
| `ch` | ChatGPT, Characters, Google Chrome, Users, Claude |
| `gc` | Google Chrome |
| `fi` | Files, Firefox, Firmware Updater, Wi-Fi, Users |
| `mi` | Microsoft Edge, Microsoft Intune, Microsoft OneDrive, Microsoft Teams (PWA), Outlook (PWA) |
| `wifi` | Wi-Fi |
| `disp` | Displays, Color |
| `termnial` | Terminal |

A search of the 84 entries takes less than 40 µs in a release build and less than 0.3 ms in a debug build.

## Start data

The crate does not start a process. It gives two forms of start data:

- `DesktopEntry::path`: the `.desktop` file. The GTK program gives it to `gio::DesktopAppInfo`. Ctrl+Enter (R2) and Shift+Enter (R13) also use it.
- `DesktopEntry::command(target)`: the argument list of `Exec` with the field codes expanded, for a caller with no GIO. `target` is one file or URL, or `None`.

Quoting rules of `Exec`, after the escapes of the key file:

1. A space or a tab separates the arguments. More than one space is one separator.
2. A double quote starts and ends a quoted part. In a quoted part, a backslash before `"`, `` ` ``, `$` or `\` gives that character. A backslash before a different character stays.
3. Single quotes and a backslash outside quotes work as in a shell. The specification does not have them, but GLib reads `Exec` with a shell parser, and real files use them, for example `sh -c '...'`.
4. Quoted and unquoted parts with no space between them make one argument: `--name="My App"` is `--name=My App`.
5. A quote that does not end makes the entry an error.

Field codes:

| Code | Expands to |
|---|---|
| `%f`, `%F`, `%u`, `%U` | The target, or nothing |
| `%i` | `--icon` and the value of `Icon` as two arguments, or nothing |
| `%c` | The localized name |
| `%k` | The path of the `.desktop` file |
| `%%` | `%` |
| `%d`, `%D`, `%n`, `%N`, `%v`, `%m`, and unknown codes | Nothing |

A code inside an argument expands in the argument: `--open=%u` is `--open=https://example.com/`. An argument that has only codes and expands to nothing is removed.

Limits: the crate passes the target with no change. It does not change a path to a `file://` URL for `%u`, or a URL to a path for `%f`. With `Terminal=true`, the caller must start a terminal: `command` does not add one.

## Browsers

`browsers()` gives the applications (not the settings pages, and not the entries with `NoDisplay=true`) that have `x-scheme-handler/http` or `x-scheme-handler/https` in `MimeType`, or the category `WebBrowser`. The order is the name, then the ID. The ID is the value for the browser of a shortcut in `shortcuts.toml` (R12), for example `microsoft-edge.desktop`.

On this desktop on 2026-11-26 the list is ChatGPT, Firefox, Google Chrome and Microsoft Edge. ChatGPT is in the list because its entry says that it handles `http` and `https`. The aliases `com.microsoft.Edge.desktop` and `com.google.Chrome.desktop` have `NoDisplay=true`, so they are not in the list.

## Change check and reload

`has_changed()` compares the modification time of each `applications` directory, each subdirectory and each `.desktop` file that owns an ID with the time at the load. A directory that did not exist at the load and exists now is a change. Thus it finds added, removed and renamed files (the time of the directory changes), and a file that an editor changed in place (the time of the file changes). It does not use inotify.

`reload()` reads all again with the same environment and settings page rule. On this desktop a load takes about 10 ms and the change check takes about 0.2 ms, in a release build.

## Usage file

`UsageStore::open(dir)` reads `dir/app-usage.tsv`. Each line is the count, a tab and the desktop file ID, in the order of the IDs. A line in a different form is ignored. A missing file gives no counts.

`record_start(id)` adds one start, makes the directory when it is missing, and writes the file. It writes `app-usage.tsv.partial` first and then renames it, so a crash does not leave half a file.

The caller gives the directory, for example `~/.cache/petlauncher/`.
