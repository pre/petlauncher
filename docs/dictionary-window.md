# The dictionary window

The module `crates/petlauncher/src/dictionary_window/` is the dictionary window of R19. Enter on a dictionary row of the launcher (`Action::OpenDictionary(word)`) opens it with the word, and the launcher closes. The data comes from `petlauncher-dict` ([dictionary.md](dictionary.md)).

## Window

| Subject | Behaviour |
|---|---|
| Number | One window for the process. A new word replaces the word in it. The program makes the window at the first word, not at its start. |
| Title | `petlauncher Dictionary`. The GNOME Shell extension moves only the window with the title `petlauncher` (ADR 0005), so it does not move this window. GNOME puts it on the monitor with the pointer. |
| Size | 720 x 820 at the first show. The owner can change the size. |
| Close | The window hides and keeps its history. The next word shows it again. |
| Launcher | The window stays when the launcher shows and hides again (R19 step 6). |
| Style | A libadwaita window. It follows the light or dark style of the system, and its CSS uses only the colors of libadwaita. |

The header bar has the back and forward buttons and the search field.

## Page of a word

1. The word as a heading.
2. The meanings in groups by word class, in the order of `petlauncher-dict`. Each group has the word class in italic and its meanings with numbers 1, 2, 3.
3. Each meaning has its definition, its examples in quotation marks, then two small lines of links:
   - the synonyms and the opposites, all of them,
   - the more general, the more specific and the "see also" words. Each list shows 4 words, and a link "N more" shows the rest of the meaning. A list with 5 words shows all 5, because a link "1 more" would take the place of the word.
4. Each base form of the word (`better` gives `good` and `well`) is one more entry below a line, with a smaller heading and the note "Base form of “better”". When the typed word is only an inflected form (`ran`), the page starts with its base form (`run`).

Each word in the links is a word of the dictionary, so a click opens it in the same window. A relation that points to the word of the entry is not shown: it would be a link to the same page. WordNet writes a quotation in a definition as `` `good' ``, and the window shows ‘good’.

The page is 600 pixels wide at most. Without its margins and the column of the numbers, the text of a meaning is about 520 pixels wide, which is about 70 characters. A long page scrolls in the window, and the header bar stays.

## Select and copy

Each meaning is one selectable label. The owner can select text with the pointer, also from the definition to the synonyms of one meaning, and Ctrl+C copies it. The headword is a selectable label too.

Limits:

- A selection cannot go from one meaning into the next, or from the heading into a meaning.
- A press in selected text starts a drag of that text (GTK). Thus a link in the selected text opens at the second click: the first click removes the selection.

Reason: a single text view would permit a selection over the full page. But a text view has no links, no style classes and no theme colors: the program would have to find the link under the pointer, set the cursor and change the colors of each tag for the dark style. The labels get the colors, the link style and the keyboard access to the links from GTK and libadwaita.

## Search field

| Action | Result |
|---|---|
| Typing | A list of at most 12 words that start with the text replaces the page. When no word starts with the text, the list says so. Typing in a different part of the window also goes to the field. |
| Down, Up | Select a word of the list. Up on the first word removes the selection. |
| Enter with a selected word | Opens that word. |
| Enter with no selected word | Opens the typed text when the dictionary finds it, also as an inflected form such as `ran`. Else it opens the first word of the list. With no list, it opens the typed text, which shows the "not found" page. |
| A click on a word | Opens that word. |

The "not found" page has the title "“launchr” is not in the dictionary" and at most 8 similar words as links. The dictionary gives only the words with a prefix, so the window asks for 300 words that start with the first half of the typed text and sorts them by the edit distance to the text. The distance counts an exchange of two adjacent letters as one change. Thus `launchr` gives `launch` and `launcher`, and `recieve` gives `receive`. A typing error in the first half of the word finds no similar word.

## Missing or bad data

When the WordNet directory is missing, the page has the title "The dictionary is not installed", the cause, the path that was tried and the command `sudo apt install wordnet-base`. When a file cannot be read or has a different format, the title is "The dictionary cannot be read" and the command is `sudo apt install --reinstall wordnet-base`. All the text is selectable. A note tells that `dictionary.wordnet_path` in `settings.toml` can give a different directory.

After the owner installs the package, the window works again after the next show of the launcher (see "Data source").

## Keys

| Key | Function |
|---|---|
| Alt+Left, Alt+Right | Back and forward. The back and forward buttons of a mouse do the same. |
| Ctrl+F, Ctrl+L | Put the focus in the search field and select its text. |
| Escape | With text in the search field: remove the text, and the page of the word shows again. With an empty field: close the window. |
| Up, Down | Move in the list of words. With no list: scroll the page. |
| Page Up, Page Down | Scroll one page. |
| Ctrl+C | Copy the selected text. |

Escape closes the window only with an empty field, so one Escape does not lose a typed word, and Escape after reading closes the window as in the launcher. When the window opens, the search field has the focus and is empty.

## History

The window keeps the words that it showed, as a browser does. A new word after Back removes the words after the current word. A word that is the current word adds nothing. Back and forward show the page at the scroll position where the owner left it. The history stays while the process runs.

## Data source

The window gets its words through the trait `DictionarySource`:

```rust
pub trait DictionarySource {
    fn lookup(&self, word: &str) -> Result<Vec<Entry>, Arc<Error>>;
    fn suggestions(&self, text: &str, limit: usize) -> Result<Vec<String>, Arc<Error>>;
}
```

`Entry` and `Error` are the types of `petlauncher-dict`. The program gives the dictionary of the `Launcher` of `petlauncher-providers` (`dictionary_lookup` and `dictionary_suggestions`, [launcher.md](launcher.md)) as the source, `LauncherDictionary` in `backend.rs`. Thus the launcher rows and the window use one open dictionary. The program installs the window one time at its start (`app.rs`):

```rust
dictionary_window::install(app, Rc::new(backend.dictionary()), move || {
    background.settings().run_in_background
});
```

The `Launcher` opens the WordNet data of `dictionary.wordnet_path` at the first use, and again when the setting changes. `~` in the setting is the home directory. A failed open stays until the launcher window opens again. Thus, after the owner installs the data, the next show of the launcher makes the window work again, and a word in the search field of the window before that show still gives the "not installed" page.

## Thread

The window calls the source on the main thread, with no worker thread. Measured in the headless test with a release build and the real data:

| Call | Time |
|---|---|
| First open of the data | 31 ms to 39 ms, one time for each process. The launcher row of the word opened it already. |
| A page: the lookup and the similar words | 0.02 ms (`launch pad`) to 0.38 ms (`run`, 57 meanings) |
| The widgets of a page | 0.3 ms to 5.6 ms (`better`, 50 meanings) |

`docs/dictionary.md` gives up to 16.5 ms for the first lookup of a word with many relations when the page cache of the operating system is empty, and 0.22 ms for the suggestions of one letter. All of these are less than one frame, except the first open. The first open comes before the window: the launcher found the row in the dictionary. A worker thread would need a request queue, old answers to drop and a page with no text for a moment, for a gain of 30 ms one time.

## Run in background

When "Run in background" is off, the process stops 1 second after a window hides, but only when no window of the program shows then (`stop.rs`). The launcher, the dictionary window and the settings window each keep the process. Thus the dictionary window stays when the launcher hides, and the process stops 1 second after the owner closes the dictionary window. The delay keeps the copied text on the clipboard, as for the launcher ([program.md](program.md)).

## Test

The unit tests cover the plain modules: `history`, `keys`, `markup` and `page`, with a source in memory. The tests of `backend` read the WordNet fixture of `petlauncher-dict` through `LauncherDictionary`.

```sh
TMPDIR=$HOME/tmp cargo test -p petlauncher
PETLAUNCHER_WORDNET_PATH=DIR gnome/extension/test/nested-test.sh \
    --program target/release/petlauncher --steps gnome/extension/test/dictionary-steps.sh --out OUT
```

`DIR` is a WordNet directory, for example the files of the package `wordnet-base`, which `apt-get download wordnet-base` and `dpkg -x` give with no root. The steps need a launcher that gives dictionary rows for `d <word>`. `dictionary-steps.sh` lists its evidence files, and `OUT/dictionary-summary.txt` has the result tables.

### Results

Environment: GNOME Shell 46.0, GTK 4.14.5, libadwaita 1.5.0, monitors 3008x1692 and 1920x1200 at scale 1, on 2026-11-26. The program had a stand-in for the dictionary rows of `petlauncher-providers`, because the launcher of this branch has no dictionary provider. Raw evidence: `~/daybook-evidence/petlauncher/dictwin/2026-11-26-dictionary-window/` on the owner's computer.

- R19 step 3: `d launcher` and Enter opened the window with "launcher", and the launcher hid. The dictionary window had the focus.
- R19 step 4: a drag over the first definition and Ctrl+C put `armament in the form of a device capable of launching a rocket` on the clipboard. A drag from the definition to the synonym gave the definition, an empty line and `Synonyms  rocket launcher`.
- R19 step 5: a click on "rocket launcher" showed that word in the same window. The back button showed "launcher" again. Alt+Right and Alt+Left did the same.
- R19 step 6: the launcher showed over the dictionary window and hid with Escape. The dictionary window stayed and got the focus again.
- The search field: `launch`, Down, Down and Enter opened "launcher". `rocke` and Enter opened "rocket". Back to "better" showed the page at its last scroll position.
- `launchr` showed the "not found" page with `launch, launcher, launce, launder, laugh, launching, laundry, lauder`. A click on "launch" opened it.
- Escape with text removed the text. Escape with an empty field closed the window.
- Missing data: the page showed the cause, `/nonexistent/wordnet` and the command. A drag and Ctrl+C put `sudo apt install wordnet-base` on the clipboard.
- The extension: in 5 shows of the dictionary window on the two monitors, the window had the same position at the map and after it, and no move after the map. The launcher kept the position of its rule in the same run.
- The program wrote no warning or critical message.
- "Run in background" off: the process stopped 1 second after the launcher hid, and the dictionary window closed with it. See "Run in background".

### Look

The screenshots are in the evidence directory, in the light and the dark style, for `launcher`, `run`, `better` with its base forms, `launch pad`, the search list, the "not found" page and the page for missing data. Changes after the first screenshots:

- The headword was selected at the first show, because GTK gave the first label the focus. The search field now gets the focus before the window shows.
- The back and forward buttons were a raised group. They are flat buttons now, as other buttons in a header bar.
- Each link was underlined, and a page with many links looked busy. The links have the color of links and no underline.
- The text was about 80 characters wide. The column is narrower now, about 70 characters.
- The link to the rest of the related words was on its own line. It is at the end of the line of related words now.
- A line broke between "See" and "also" and inside a word of two parts.
- Back to a long page showed its top, not the place where the owner left it.

### Not tested

- The live session and the real monitors of the owner.
- The real dictionary rows of `petlauncher-providers`: the launcher of this branch has none, and the test used a stand-in row.
- The keyboard access to the links (Tab), the mouse buttons for back and forward, Page Up, and the link "N more".
- Typing while a label has the focus (the text must go to the search field), and a click on a word of the search list.
- A WordNet directory with incomplete or bad files: the unit tests cover the text of the page, the headless test only the missing directory.
- A change of `dictionary.wordnet_path` while the window shows.
- Input methods other than the keyboard layout `us`.
