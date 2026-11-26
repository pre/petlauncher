# File search

The crate `petlauncher-index` makes the index for the search by file name (requirements R14 and R21). The core gives it the text after the keyword, for example `palette-zoom` for the query `f palette-zoom`. This document gives the rules of the crate.

## Scope

The scope tells what the index contains. It has four parts:

| Part | Contents |
|---|---|
| Roots | The directories to scan. |
| Ignore entries | The entries of the ignore list. |
| Home directory | The directory that `~` means. |
| Ignore files outside the roots | On: obey the global git excludes file and the ignore files in the parent directories of the roots, as `rg` does. The program sets it on. The tests set it off, so that they do not read the git settings of the owner. |

A root is `~`, a path that starts with `~/`, or an absolute path. Other text is relative to the home directory. When two roots overlap, the index contains each path one time.

### Ignore entries

An entry has one of three forms. The crate finds the form with these steps:

1. Remove the spaces at the two ends, and the slashes at the end. Thus `node_modules/` is the same as `node_modules`, and `/` stays `/`.
2. If the entry is `~` or contains a `/`, it is a path.
3. Else, if the entry contains `*`, `?` or `[`, it is a pattern.
4. Else, it is a directory name.

| Form | Examples | What it matches |
|---|---|---|
| Path | `~/tmp`, `~/go/pkg/mod`, `/mnt/data`, `go/pkg/mod` | Only that file or directory. `~/` is the home directory. A path that does not start with `/` or `~/` is relative to the home directory, so `go/pkg/mod` is the same as `~/go/pkg/mod`. `*`, `?` and `[` have no special meaning in a path. |
| Directory name | `node_modules`, `target`, `__pycache__`, `git-worktrees` | Each directory with that name, at all depths. Not a file with that name. |
| Pattern | `*-wt` | Each directory whose name matches the pattern, at all depths. Not a file. The syntax is that of the crate `globset`: `*`, `?`, `[abc]`, `[!abc]`, and `{a,b}`. |

The match is case-sensitive. The order of the entries has no effect.

An empty entry, or a pattern that `globset` cannot read, is an error. `FileIndex::open` and `FileIndex::set_scope` give the error, and `set_scope` keeps the old scope. A settings window can test one entry with `IgnoreEntry::parse`.

The ignore entries apply below the roots, not to a root itself. Thus a root `~/tmp/project` is scanned when the list contains `~/tmp`. The scan does not enter an ignored directory.

### Initial ignore list of R14

| Entry | Form | Effect |
|---|---|---|
| `node_modules`, `target`, `__pycache__` | Directory name | Skips each directory with that name. |
| `~/go/pkg/mod` | Path | Skips the Go module cache of the home directory. `~/go/src` and a `go/pkg/mod` in a different place stay. |
| `~/tmp` | Path | Skips `~/tmp`. `~/dev/tmp` and `~/tmp2` stay. |
| `git-worktrees`, `*-wt` | Directory name, pattern | Skips the extra git worktrees. The main checkout, for example `~/dev/petlauncher`, stays. |

## Rules of the scan

The scan uses the parallel walker of the crate `ignore` from ripgrep.

- Hidden files and directories (the name starts with `.`) are not in the index. A root can be hidden.
- `.gitignore` files have effect only inside a git repository, as in `rg`. The scan also obeys `.git/info/exclude`, and the global git excludes file: the `core.excludesFile` of the git settings, or else `~/.config/git/ignore`. The global file also has effect only inside a git repository.
- The scan also obeys `.ignore` files, as `rg` does. This is the default of the crate `ignore`.
- When "ignore files outside the roots" is on, the ignore files in the parent directories of a root also apply. Example: a root inside a git repository obeys the `.gitignore` of the repository.
- The scan does not follow symbolic links. A symbolic link is an entry of the index, but the scan does not enter it. A root that is a symbolic link to a directory is scanned.
- Sockets, pipes and devices are not in the index.
- The scan does not stop at a mount point.
- A directory or a file that the scan cannot read does not stop the scan. `Index::errors()` gives their number.
- The scan reads the modification time of each entry, for the rank.

### Directories

The index contains directories as well as files, because a directory row is useful: `f H3-IMAGE` finds the folder `H3-IMAGE`. `search` gives files and directories. `search_files` gives only files and symbolic links. The two use the same index, so the choice costs nothing.

## Match and rank

### Match

1. The text is split at white space into words. A text with no words gives no results.
2. Case: when the text has no upper-case letter, the search ignores case. When the text has an upper-case letter, the case must be the same.
3. Each word must be in the name of the file or directory. The directory part of the path is not searched.

To ignore case, the crate changes each letter to lower case. A letter whose lower case has a different UTF-8 length stays as it is, for example `İ` (U+0130). In a name that is not valid UTF-8, only the ASCII letters change.

### Rank

Each word gets a class from the best place where it is in the name:

| Class | Value | Condition | Example for `palette` |
|---|---|---|---|
| Exact | 0 | The name is the word. | `palette` |
| Stem | 1 | The name before its last dot is the word. | `palette.png` |
| Prefix | 2 | The name starts with the word. | `palette-x` |
| Word start | 3 | A word of the name starts with the word. A word of the name starts after a character that is not a letter or a digit, or at a change from a lower-case to an upper-case ASCII letter. | `my-palette-x`, `MyPalette` |
| Inside | 4 | The word is in the name at a different place. | `mypalette` |

The score of an entry is the sum of the values of its words. The results are in this order:

1. Lower score.
2. Shorter name, in bytes.
3. Newer modification time.
4. Shorter full path.
5. Full path, in byte order.

`search(text, limit)` gives the first `limit` results.

The acceptance of R14 is a stem match: `palette-zoom` finds `palette-zoom.png` in the class Stem.

## Index in memory

`FileIndex` keeps an `Arc<Index>`. The index has:

- each directory path one time, in one byte buffer, with a list of end offsets,
- all names in one byte buffer, and a second buffer with the same names in lower case. The two buffers have the same byte positions, so one offset serves the two,
- one entry of 16 bytes for each file, link or directory: the number of its directory, the end of its name, the modification time in seconds, and the kind.

The search does one substring search of the longest word through the whole lower-case buffer. It examines only the entries where that word is found.

## Stored file

The file is `file-index.bin` in the cache directory that the program gives, for example `~/.cache/petlauncher/file-index.bin`. All numbers are little-endian.

| Part | Contents |
|---|---|
| Magic | 8 bytes: `PLINDEX` and a zero byte. |
| Format version | `u32`, now 1. |
| Checksum | `u32`, CRC-32 (IEEE) of the body. |
| Body: scope | The home directory, the switch for the ignore files outside the roots (`u8`), the roots, and the ignore entries. Each text is a `u32` length and the bytes. Each list is a `u32` count and the texts. |
| Body: scan time | `u64`, nanoseconds after the Unix epoch. 0: no scan. |
| Body: errors | `u32`. |
| Body: directory paths | A `u32` length and the bytes of all paths. |
| Body: directory ends | A `u32` count and a `u32` end offset for each directory. |
| Body: names | A `u32` length and the bytes of all names. |
| Body: entries | A `u32` count and 13 bytes for each entry: directory number `u32`, name end `u32`, modification time `u32` (seconds), kind `u8` (0: file or link, 1: directory). |

The lower-case names are not in the file. The load makes them again.

The load does not trust the file. It ignores the file when one of these is true:

- The magic or the format version is different.
- The checksum is not correct.
- The scope is different. The comparison is on the text, so a change of the order of the ignore entries also makes the file invalid.
- The structure is wrong: an offset that is not after the one before it, a last offset that is not the length of its buffer, a directory number that is too large, a kind that is not 0 or 1, or bytes after the entries.

The index is then empty until the next scan, and that scan replaces the file.

The write is atomic. The crate writes a temporary file in the same directory, calls `fsync`, and renames the file to `file-index.bin`.

## Rescan rule

The crate has no inotify watches. The program calls `FileIndex` at these times:

| Time | Call | Effect |
|---|---|---|
| Start | `FileIndex::open(scope, cache_dir)`, then `rescan()` | `open` loads the stored file, so the first search does not wait for a scan. `rescan` updates the index in the background. |
| The window opens | `if index.is_older_than(age) { index.rescan(); }` | The program selects `age`, for example 60 seconds. |
| The roots or the ignore entries change (R21) | `set_scope(scope)` | Stops a running scan and starts a new scan immediately. |
| End | Drop the `FileIndex` | Stops a running scan. A stopped scan does not change the snapshot or the file. |

Rules:

- Not more than one scan runs at a time. `rescan` returns false and does nothing when a scan runs. `set_scope` stops the running scan and waits until it ends, then it starts the new scan.
- A search is possible from each thread at all times. It takes the current snapshot and does not wait for a scan. When a scan ends, its index replaces the snapshot and then the stored file. A search that started before the replacement uses the old index to its end.
- Until the scan after `set_scope` ends, a search uses the index of the old scope.
- `is_older_than(age)` is true when no scan made the index, or when the scan that made it started `age` or more ago.
- `last_error()` gives the error of the last write of the stored file. The index in memory is correct also when the write fails.
- `wait()` waits until the running scan ends. It is for the tests and for the end of the program.
- `FileIndex::open_with_listener(scope, cache_dir, on_update)` is the same as `open`, and calls `on_update` on the scan thread each time a scan replaces the snapshot, before the write of the stored file. A stopped scan does not call it. The program uses it to search the query again.

## Measurements

Measured on 2026-11-26 on the owner's computer (32 processor threads, ext4), with the directory data in the disk cache of the operating system. Release build.

Scan of the home directory with the initial ignore list, read-only, with the example `scan_stats`:

| Quantity | Value |
|---|---|
| Entries | 121 782: 99 795 regular files, 21 884 directories, 103 symbolic links |
| Full paths of the regular files as plain text | 9.97 MB |
| Index in memory | 8.14 MB |
| Scan, automatic thread count (12) | 0.12 s to 0.15 s |
| Scan, 32 threads | 0.11 s to 0.12 s |
| Scan, 1 thread | 0.59 s to 0.70 s |
| Stored file | 5.60 MB, load 5 ms |
| Search `palette-zoom`, `readme`, `e` | 0.06 ms, 0.16 ms, 4.5 ms (median) |

`rg --files` with the same rules found 99 910 files in 0.05 s to 0.06 s. The index contains each of these files. It also contains the 103 symbolic links, which `rg --files` does not list. The scan takes more time than `rg`, because it also reads the modification time of each entry and builds the index.

The benchmark `cargo bench -p petlauncher-index` makes an index with the size of R14: 91 755 files in 16 261 directories, 108 007 entries. Its index in memory is 6.60 MB. A search takes 0.03 ms to 0.4 ms, and 5.5 ms for the one-letter text `e`, which matches almost all names. The stored file is 4.64 MB and loads in 5 ms.
