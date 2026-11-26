# petlauncher-index

The file index for the search by file name (R14, R21). It scans the roots with the ignore rules, keeps a compact index in memory, stores it in the cache directory, and scans again in the background. It has no dependency on GTK, GLib or the other petlauncher crates.

The ignore rules, the match and rank rule, the stored file format and the rescan rule are in [docs/file-search.md](../../docs/file-search.md).

- Tests: `TMPDIR=$HOME/tmp cargo test -p petlauncher-index`
- Search benchmark: `TMPDIR=$HOME/tmp cargo bench -p petlauncher-index`
- Manual scan of a home directory, with no file names in the output: `TMPDIR=$HOME/tmp cargo run --release -p petlauncher-index --example scan_stats -- ~`
