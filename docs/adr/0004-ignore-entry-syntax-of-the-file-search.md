# Own syntax for the ignore entries of the file search

Status: accepted.

The ignore list of the file search is in `settings.toml`, so its syntax is a user format that is hard to change later. An entry is a path when it is `~` or contains a `/`, a pattern when it contains `*`, `?` or `[`, and else a directory name. A path that does not start with `/` or `~/` is relative to the home directory. Names and patterns match directories only, and a path matches one file or directory.

## Considered Options

- Gitignore syntax, for example `/tmp/` and `node_modules/`: it is well known, but it has no `~`, a root outside the home directory needs its own anchor, and `!` and `**` add rules that the settings window must show.
- A path relative to each root: the same for the root `~`, but an entry then means different directories for different roots.
