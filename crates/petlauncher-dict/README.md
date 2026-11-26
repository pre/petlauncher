# petlauncher-dict

Reader for the Princeton WordNet 3.0 database files, for the dictionary of petlauncher (R19). Plain Rust, with no GTK, GLib or D-Bus dependency.

- `Dictionary::open(path)` reads the index of the WordNet directory, usually `/usr/share/wordnet` from the Ubuntu package `wordnet-base`.
- `Dictionary::lookup(text)` gives the meanings of a word and of its base forms.
- `Dictionary::suggestions(text, limit)` gives the words that start with a text.

Try it with the real data:

```sh
cargo run --release -p petlauncher-dict --example wordnet -- /usr/share/wordnet launcher
```

The data source, the file format, the morphology rules, the measurements and the limits are in [docs/dictionary.md](../../docs/dictionary.md).
