# Settings files

petlauncher keeps its settings in two TOML files in `~/.config/petlauncher/`.
When `XDG_CONFIG_HOME` is set to an absolute path, the directory is
`$XDG_CONFIG_HOME/petlauncher/`.

| File | Contents |
|---|---|
| `settings.toml` | The settings |
| `shortcuts.toml` | The shortcuts, with one `[[shortcut]]` block for each shortcut |

A file that does not exist gives the initial values. When the settings window
saves a change and the file does not exist, it writes a new file with a
comment for each key.

## Change a file in an editor

petlauncher reads a file again approximately 0.1 seconds after you save it.
If the file has an error, the old values stay in effect and petlauncher shows
the error with the file name and the line number.

The settings window changes only the value in question. Your comments, empty
lines and the order of the items stay. petlauncher writes a file in one step:
it writes a temporary file in the same directory, then renames it. If the
file is a symbolic link, petlauncher writes the file that the link points to,
and the link stays.

## The settings window

The settings window ([settings-window.md](settings-window.md)) shows the
values of the two files and changes them with the same rules: a change
writes only its value. While a file has an error, the window shows the error
with the line and does not change the file. When you save a file in an
editor, the window shows the new values.

## settings.toml

| Key | Meaning | Initial value |
|---|---|---|
| `close_on_focus_loss` | `true`: the window closes when a different window gets the focus. | `true` |
| `run_in_background` | `true`: petlauncher stays in operation after the window closes, so the window opens quickly. petlauncher also starts at login. | `true` |
| `show_tray_icon` | `true`: an icon shows in the top bar. | `true` |
| `include_applications` | `true`: the installed applications are in the search results. | `true` |
| `hotkey` | The key that shows and hides petlauncher, in the GNOME key syntax. GNOME keeps the key that is in effect. petlauncher writes this key to GNOME one time, at the first start, when no other key binding has it (ADR 0008). The settings window writes a new key to GNOME and to this setting. | `"<Super>space"` |
| `window_width` | The width of the window in pixels, from 300 to 7680. | `750` |
| `keep_query` | How long petlauncher keeps the last query after the window closes: `"disabled"`, `"forever"`, a number of seconds such as `"30s"`, or a number of minutes such as `"5m"`. | `"30s"` |

### The table `[file_search]`

| Key | Meaning | Initial value |
|---|---|---|
| `keyword` | The query `<keyword> <text>` finds the files whose names match `<text>`. | `"f"` |
| `directories` | The directories that the file search examines. `~` is the home directory. | `["~"]` |
| `ignore` | The items that the file search does not examine. An item is a path such as `"~/tmp"`, a directory name such as `"node_modules"`, or a pattern for a directory name such as `"*-wt"`. | `["node_modules", "target", "__pycache__", "~/go/pkg/mod", "~/tmp", "git-worktrees", "*-wt"]` |

A file with no `ignore` key has the initial list. When the settings window
adds an item and the file has no `ignore` key, it writes the initial list with
the new item. The same is true for `directories`.

### The table `[dictionary]`

| Key | Meaning | Initial value |
|---|---|---|
| `keyword` | The query `<keyword> <word>` finds the word in the dictionary. | `"d"` |
| `wordnet_path` | The directory with the WordNet files, for example `index.noun` and `data.noun`. | `"/usr/share/wordnet"` |

### Example

```toml
# My settings.

window_width = 900      # wider than the initial value
keep_query = "5m"

[file_search]
keyword = "f"
directories = ["~"]
ignore = [
    "node_modules",
    "target",
    "__pycache__",
    "~/go/pkg/mod",
    "~/tmp",
    "git-worktrees",
    "*-wt",
    "~/lolcat-evidence",   # too many files
]

[dictionary]
keyword = "d"
```

The keys that this example does not have keep their initial values.

## shortcuts.toml

Each `[[shortcut]]` block is one shortcut. The order of the blocks is the
order of the shortcuts.

| Key | Meaning | Initial value |
|---|---|---|
| `name` | The text of the row. | `""` |
| `keyword` | The word that starts the shortcut. The query `jira 123` gives the argument `123`. | `""` |
| `cmd` | A URL or a script. `%s` becomes the argument. A URL opens in the browser. Other text runs as a script, and the script also gets the argument as `$1`. | `""` |
| `cmd_without_argument` | The URL or the script for a query with no argument. Empty: `run_without_argument` decides. | `""` |
| `run_without_argument` | `true`: a query with no argument runs `cmd`. `false`: the shortcut needs an argument, and Enter with no argument does nothing. | `false` |
| `is_default_search` | `true`: the shortcut also shows as a row when no other row matches the query, with the full query as the argument. | `false` |
| `icon` | The icon of the row: an icon name or a file path. | `""` |
| `browser` | The browser for a URL: `{ app = "<desktop entry>" }` for an installed browser, or `{ command = "<command>" }` for a different command. petlauncher adds the URL as the last argument of the command. No `browser` key: the default browser. | no key |

A script with more than one line is easy to read as a TOML multi-line string,
between `"""` and `"""`.

When the settings window moves or removes a shortcut, the comment lines
directly above its `[[shortcut]]` line go with it. A comment that an empty
line separates from the `[[shortcut]]` line stays at its place in the file.
Thus, write a comment about a shortcut directly above its `[[shortcut]]`
line.

The settings window does not change a file that has the shortcuts as an
inline list, `shortcut = [{ ... }]`, because such a change cannot keep the
comments in the list. petlauncher reads such a file correctly.

### Example

```toml
# My shortcuts.

[[shortcut]]
name = "Web search"
keyword = "g"
cmd = "https://search.example.com/?q=%s"
is_default_search = true

# A pull request by its number, or the list with no number.
[[shortcut]]
name = "Pull requests"
keyword = "pr"
cmd = "https://github.com/example/project/pull/%s"
cmd_without_argument = "https://github.com/example/project/pulls"
browser = { app = "microsoft-edge.desktop" }

[[shortcut]]
name = "Local board"
keyword = "board"
cmd = "http://localhost:9999/#board"
run_without_argument = true

[[shortcut]]
name = "Review"
keyword = "rv"
cmd = """
#!/bin/sh
exec example-browser --profile work "https://review.example.com/pull/$1"
"""
```

## Errors and warnings

An error stops the file. When you save a file with an error, the old values
stay in effect and petlauncher shows the error. The settings window does not
save a change that gives an error. A warning does not stop a file or a
change. petlauncher shows the warning.

| Check | Kind | Reason |
|---|---|---|
| The file is not valid TOML. | Error | petlauncher cannot read the values. |
| A value has the wrong type, for example `window_width = "wide"`. | Error | petlauncher cannot read the value. |
| `window_width` is not from 300 to 7680. | Error | A narrower window cannot show a row of results, and 7680 pixels is the width of an 8K monitor. A value out of this range is almost always a typing mistake, for example `75` for `750`. |
| A keyword is empty: of a shortcut, of the file search or of the dictionary. | Error | An empty keyword cannot start a query. |
| A keyword contains a space. | Error | A space separates the keyword from the argument, so a query cannot start with this keyword. |
| Two shortcuts have the same keyword. | Warning | petlauncher can use the two shortcuts, but this is probably a mistake. |
| A shortcut has the keyword of the file search or of the dictionary. | Warning | The two items are in different files. Neither file alone is wrong, so petlauncher stops neither file. |
| The file search and the dictionary have the same keyword. | Warning | petlauncher can use the two searches, but this is probably a mistake. |
| The file has a key that petlauncher does not know, for example `windw_width`. | Warning | The key has no effect. A later version of petlauncher can know the key, so the key does not stop the file. |

A conflict between the two files is always a warning, because then no single
file is wrong and petlauncher cannot select which file to stop.
