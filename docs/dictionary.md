# Dictionary

The query `d <word>` finds an English word in a dictionary on the disk (R19). The crate `petlauncher-dict` reads the dictionary data. It is plain Rust and has no dependency on GTK, GLib, GNOME, D-Bus or the other petlauncher crates. It works on Linux and other Unix systems only, because it reads the data files with `pread` (`std::os::unix::fs::FileExt`).

## Data source

The data is Princeton WordNet 3.0 from the Ubuntu package `wordnet-base`, version 1:3.0-37 (Ubuntu 24.04, section universe). The installed size is 28 496 kB. The words are from 2006, so the newest words are not in it.

The package puts the database files in `/usr/share/wordnet`. This is the initial value of the setting `dictionary.wordnet_path`. A different WordNet edition in the same file format can replace it: set `dictionary.wordnet_path` to its directory.

The data has the WordNet 3.0 license of Princeton University. The data is not in the repository, so its license does not touch the MIT license of the code. The test fixtures contain invented definitions only.

## Installation

Install the package:

```sh
sudo apt install wordnet-base
```

The package `wordnet` (the commands `wn` and `wnb`) is not necessary.

When the directory is missing, `Dictionary::open` gives `Error::NotInstalled`. Its text is `the package wordnet-base is not installed: there is no directory /usr/share/wordnet`. When files are missing, it gives `Error::Incomplete` with the names of the files. The user interface can show the text of the error.

To see the result for a word in a terminal:

```sh
cargo run --release -p petlauncher-dict --example wordnet -- /usr/share/wordnet launcher launch
```

## Files

| File | Use | Necessary |
|---|---|---|
| `index.noun`, `index.verb`, `index.adj`, `index.adv` | All words of each word class, with the byte offsets of their synsets in the data file | Yes |
| `data.noun`, `data.verb`, `data.adj`, `data.adv` | One synset on each line: the words, the pointers to other synsets, and the gloss | Yes |
| `noun.exc`, `verb.exc`, `adj.exc`, `adv.exc` | Irregular forms and their base forms, for example `ran run` | Yes |
| `cntlist.rev` | How many times the semantic concordance texts tag each sense. Used only for the order of the suggestions. | No |

The package has more files (`sentidx.vrb`, `sents.vrb`). The reader does not use them.

## File format parts that the reader uses

The format is in the manual page `wndb(5WN)`, which is in the package. The lines at the start of each index and data file are the license text. They start with two spaces, and the reader skips them.

An index line has these fields, with one space between them:

```
lemma pos synset_cnt p_cnt [ptr_symbol...] sense_cnt tagsense_cnt synset_offset...
launcher n 2 3 @ ~ + 2 1 03647520 02982232
```

The lemma is in lower case, with `_` between the words of a collocation. The offsets are in the order of the senses, the most frequent sense first. `tagsense_cnt` is the number of senses that the semantic concordance texts tag. The reader uses the lemma, `tagsense_cnt` and the offsets.

A data line has these fields:

```
synset_offset lex_filenum ss_type w_cnt word lex_id [word lex_id...] p_cnt [ptr...] [frames...] | gloss
```

- `w_cnt` is a hexadecimal number. Each word is as the lexicographer wrote it, with its case and with `_` for a space.
- In `data.adj`, a word can have a syntactic marker: `(a)` before a noun, `(p)` in the predicate (after a verb such as `be`), `(ip)` immediately after a noun. The reader removes the marker. The real data has 596 `(a)`, 430 `(p)` and 29 `(ip)` markers.
- `ss_type` `s` is an adjective satellite. The reader shows it as an adjective.
- Each pointer is `pointer_symbol synset_offset pos source/target`. `source/target` is two hexadecimal word numbers. `0000` is a pointer from the full synset to the full synset. A different value is a pointer from one word to one word. A word number starts at 1.
- The reader uses these pointer symbols: `!` antonym, `^` see also, `@` and `@i` hypernym (the more general word, also of an instance), `~` and `~i` hyponym (the more specific word). It ignores the other symbols and the verb frames.
- The reader does a check that the offset at the start of the line is the offset in the index.

An exception line is an inflected form and one or more base forms, for example `axes ax axis`.

A `cntlist.rev` line is `sense_key sense_number tag_cnt`, for example `launch%2:38:00:: 1 20`. The sense key starts with the lemma and `%`.

### Gloss

A gloss is the definition, then the examples in quotation marks, with `;` between the parts:

```
propel with force; "launch the space shuttle"; "Launch a ship"
```

The reader splits the gloss at each `;` that is not in quotation marks. A part that starts with a quotation mark is an example, and all parts after the first example are examples. Some glosses do not obey the form. The reader also accepts these forms:

- `:` or `,` between the definition and the first example.
- A missing first quotation mark on an example after the definition.
- The name of an author after an example, for example `"..."- Shakespeare`. The name stays at the end of the example.
- Two examples with `,` between them in one part.

On the real data, 32 921 of the 32 930 glosses with quotation marks give one or more examples. No example contains a quotation mark. 16 definitions keep quotation marks. Most of them are correct, for example `promise of reward as in "carrot and stick"`. Some are errors of the data, for example an example after `(of a ball)` with no `;` between them.

## Memory form

`Dictionary::open` reads the four index files into one table in memory. The table has one row for each word, also for a word in two or more word classes. Each row has the synset offsets of each class, the `tagsense_cnt` of each class, and the frequency from `cntlist.rev`. All words are in one string, so the table has few allocations.

The data files stay on the disk. A lookup reads each synset with one `pread` call at its offset. Thus the 22 MB of synset text is not in memory, and the page cache of the operating system keeps the parts that were read.

Reasons: the table gives a lookup and a prefix search with a binary search, and an order of the suggestions by frequency. A binary search in the index files on the disk needs no memory, but each suggestion list then reads all matching lines from the disk.

## Lookup

`Dictionary::lookup(text)`:

1. Changes the text to the index form: lower case, and `_` between the words. Spaces before and after the text have no effect.
2. If the index has the word, the first entry is that word, with `Found::Exact`.
3. For each word class, the morphology finds the base forms of the text. Each base form that is a word of that class is one more entry, with `Found::BaseForm`. A base form of two or more classes is one entry with the meanings of those classes.

The meanings of an entry are in this order. The word class with the larger `tagsense_cnt` is first. For equal counts, the order is noun, verb, adjective, adverb. In each class, the senses keep the order of the index, so the most frequent sense is first. For example, the verb meanings of `launch` come before its noun meanings.

Each meaning has the word class, the definition, the examples, the synonyms (the other words of the synset), and the optional relations: antonyms, see also, hypernyms and hyponyms. A pointer from one word applies only to that word. For example, in the synset `big, large`, an antonym pointer from `large` is not an antonym of `big`.

The words of a meaning have a space for `_` and keep the case of the data, for example `New York City`. The word of an entry is in lower case. On the real data, each word of each synset is a word of the index of its class, after the marker is removed and the word is in lower case. Thus each synonym and each related word is a word that a lookup finds, and the dictionary window can show it as a link.

## Morphology

The rules are those of the WordNet morphology (Morphy).

1. If the exception list of the class has the word, the base forms are the forms of the list that are words of the class. The suffix rules are not used.
2. An adverb has no suffix rules.
3. A noun that ends in `ful` gets the rules on the part before `ful`, and the base form gets `ful` again: `boxesful` gives `boxful`.
4. A noun that ends in `ss` or has two letters or fewer gets no rule.
5. The reader uses the rules of the class in the order of the table. The first rule that gives a word of the class gives the base form. For example, `hoped` gives `hope` and not `hop`.
6. A collocation gets the base form of each part, with the exception list or the rules. If the result is a word of the class, it is the base form: `looked up` gives `look up`.

| Class | Rules: ending of the inflected form, then ending of the base form |
|---|---|
| Noun | `s` none, `ses` `s`, `xes` `x`, `zes` `z`, `ches` `ch`, `shes` `sh`, `men` `man`, `ies` `y` |
| Verb | `s` none, `ies` `y`, `es` `e`, `es` none, `ed` `e`, `ed` none, `ing` `e`, `ing` none |
| Adjective | `er` none, `est` none, `er` `e`, `est` `e` |

Examples with the real data: `launchers` gives `launcher`, `ran` gives `run` (from `verb.exc`), and `better` gives the word `better` and the base forms `good` and `well` (from `adj.exc` and `adv.exc`).

## Suggestions

`Dictionary::suggestions(text, limit)` gives at most `limit` words that start with the text, in the index form. The order is:

1. The text itself, if it is a word.
2. The words with a larger frequency in `cntlist.rev`.
3. The other words in alphabetical order (byte order of the index form).

A space at the end of the text asks for the collocations: `launch ` gives `launch area` and `launch pad`. An empty text or a limit of 0 gives no words. Without `cntlist.rev`, the order is the text itself, then alphabetical order.

With the real data, `launch` gives `launch, launcher, launching, launch area, launch pad, launching pad, launching site, launchpad`, and `d` gives `d, do, day, develop, determine, die, drive, down, door, development`.

## Measurements

Measured on 2026-11-26 with the example program `wordnet` (release build) on the owner's desktop: AMD Ryzen AI Max+ PRO 395, ext4 on an LVM logical volume. The times of a lookup and of a suggestion list are the mean of 100 calls after a first call.

| Quantity | Value |
|---|---|
| Words in the index | 147 306, with 206 941 synset offsets |
| Open, files in the page cache | 46 ms to 50 ms |
| Open, page cache of the WordNet files empty | 52 ms to 54 ms |
| Resident memory after open | 7.7 MB more than before open. The table is 6.1 MB (calculated from its sizes). The exception lists and the allocator use the rest. These two parts were not measured separately. |
| Lookup `launcher` | 0.014 ms (first call with an empty page cache: 0.45 ms) |
| Lookup `launch` | 0.037 ms (first call with an empty page cache: 1.1 ms) |
| Lookup of words with many relations: `city`, `person`, `break` | 0.5 ms to 0.9 ms (first call with an empty page cache: up to 16.5 ms) |
| Suggestions `launch` | 0.001 ms |
| Suggestions of one letter: `s`, `a`, `d` | 0.09 ms to 0.22 ms |

The page cache was emptied for each WordNet file with `dd iflag=nocache count=0`, and `fincore` showed 0 bytes in the cache. The open time is almost the same with and without the page cache, so the parse of the index is the larger part of it, not the disk.

## Limits

- The morphology gives one base form for each rule set, the first rule that gives a word. The WordNet library does the same. A word with two possible regular base forms gets only the first.
- A collocation is split only at `_`, not at `-`. The special rule of WordNet for a verb with a preposition is not implemented separately. The rule for each part gives the correct result for `asking for`, `gave up`, `ran into` and `takes off` with the real data. Other cases were not examined.
- Antonyms are only the direct antonym pointers. WordNet shows "indirect" antonyms of a satellite adjective through its head adjective. The reader does not.
- A relation can contain the word itself, for example the hypernym of sense 3 of `launch` is `launch`. The data has this.
- The order of the word classes uses `tagsense_cnt`, not the full tag counts of each sense.
- The index is read when the dictionary opens. If the data files change while the dictionary is open, the offsets can be wrong. A package upgrade replaces the files, and the open file handles keep the old files, so a lookup stays correct until the dictionary opens again.

## Tests

The tests use the fixture in `crates/petlauncher-dict/tests/fixtures/wordnet/`. It has the WordNet file format, with invented definitions. Each data line starts with its own byte offset, and the index lines and the pointers contain these offsets. If you change a line of a data file, you must correct the offsets of all later lines and all references to them. A lookup of a wrong offset gives `Error::Format`, so the tests find a wrong offset.

The tests in `tests/real_data.rs` and the test `real_every_synset_word_is_a_word_of_its_class` read the real data. They run only when the environment variable `PETLAUNCHER_WORDNET_PATH` gives the WordNet directory:

```sh
PETLAUNCHER_WORDNET_PATH=/usr/share/wordnet TMPDIR=$HOME/tmp cargo test -p petlauncher-dict
```

Without the variable, these tests pass and do nothing.
