//! Reader for the WordNet 3.0 database files, for the dictionary of
//! petlauncher (R19). See `docs/dictionary.md` for the data source, the file
//! format and the limits.

mod data;
mod error;
mod gloss;
mod index;
mod morph;

use std::cmp::Reverse;
use std::fmt;
use std::path::Path;

use data::DataFile;
pub use error::Error;
use index::Index;
use morph::Exceptions;

/// The word class of a meaning. WordNet has these four classes only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WordClass {
    Noun,
    Verb,
    Adjective,
    Adverb,
}

impl WordClass {
    /// The classes in the order of the WordNet files.
    pub(crate) const ALL: [WordClass; 4] = [
        WordClass::Noun,
        WordClass::Verb,
        WordClass::Adjective,
        WordClass::Adverb,
    ];

    /// The name for the user interface: "noun", "verb", "adjective" or "adverb".
    pub fn name(self) -> &'static str {
        match self {
            WordClass::Noun => "noun",
            WordClass::Verb => "verb",
            WordClass::Adjective => "adjective",
            WordClass::Adverb => "adverb",
        }
    }

    /// The end of the file names: `index.noun`, `data.adj`, `adv.exc`.
    pub(crate) fn file_suffix(self) -> &'static str {
        match self {
            WordClass::Noun => "noun",
            WordClass::Verb => "verb",
            WordClass::Adjective => "adj",
            WordClass::Adverb => "adv",
        }
    }

    /// The class of a pointer target. `s` is an adjective satellite.
    pub(crate) fn from_letter(letter: &str) -> Option<WordClass> {
        match letter {
            "n" => Some(WordClass::Noun),
            "v" => Some(WordClass::Verb),
            "a" | "s" => Some(WordClass::Adjective),
            "r" => Some(WordClass::Adverb),
            _ => None,
        }
    }
}

impl fmt::Display for WordClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How the dictionary found the word of an [`Entry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    /// The typed text is the word.
    Exact,
    /// The word is the base form of the typed text, for example `run` for `ran`.
    BaseForm,
}

/// A word of the dictionary with its meanings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The word in lower case, with spaces between the parts of a collocation.
    pub word: String,
    pub found: Found,
    /// The meanings, word class by word class. The class with the most
    /// frequent meanings is first. In each class, the most frequent meaning is first.
    pub meanings: Vec<Meaning>,
}

/// One meaning of a word (a WordNet synset).
///
/// Each word in the lists is a word of the dictionary, so a lookup of it
/// gives an [`Entry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Meaning {
    pub word_class: WordClass,
    pub definition: String,
    /// Example sentences, without the quotation marks.
    pub examples: Vec<String>,
    /// The other words with this meaning.
    pub synonyms: Vec<String>,
    /// Words with the opposite meaning.
    pub antonyms: Vec<String>,
    /// Related words ("see also").
    pub see_also: Vec<String>,
    /// More general words.
    pub hypernyms: Vec<String>,
    /// More specific words.
    pub hyponyms: Vec<String>,
}

/// The files that a WordNet directory must have.
const REQUIRED_FILES: [&str; 12] = [
    "index.noun",
    "index.verb",
    "index.adj",
    "index.adv",
    "data.noun",
    "data.verb",
    "data.adj",
    "data.adv",
    "noun.exc",
    "verb.exc",
    "adj.exc",
    "adv.exc",
];

/// An open WordNet database. The index is in memory. A lookup reads its
/// synsets from the data files.
pub struct Dictionary {
    index: Index,
    exceptions: Exceptions,
    data: [DataFile; 4],
}

impl fmt::Debug for Dictionary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dictionary")
            .field("words", &self.index.len())
            .finish_non_exhaustive()
    }
}

impl Dictionary {
    /// Opens the WordNet files in `path` and reads the index into memory.
    pub fn open(path: impl AsRef<Path>) -> Result<Dictionary, Error> {
        let path = path.as_ref();
        if !path.is_dir() {
            return Err(Error::NotInstalled {
                path: path.to_owned(),
            });
        }
        let missing: Vec<&'static str> = REQUIRED_FILES
            .into_iter()
            .filter(|file| !path.join(file).is_file())
            .collect();
        if !missing.is_empty() {
            return Err(Error::Incomplete {
                path: path.to_owned(),
                missing,
            });
        }
        let [noun, verb, adjective, adverb] = WordClass::ALL
            .map(|class| DataFile::open(path.join(format!("data.{}", class.file_suffix()))));
        Ok(Dictionary {
            index: Index::read(path)?,
            exceptions: Exceptions::read(path)?,
            data: [noun?, verb?, adjective?, adverb?],
        })
    }

    /// Finds the typed text and its base forms. The case of the text and the
    /// spaces before and after it have no effect. Gives no entries when the
    /// dictionary does not have the word.
    ///
    /// The typed word is the first entry if the dictionary has it. Then each
    /// base form that the morphology finds is an entry, with the meanings of
    /// the word classes in which it is the base form.
    pub fn lookup(&self, text: &str) -> Result<Vec<Entry>, Error> {
        let word = index_form(text);
        let mut entries = Vec::new();
        if word.is_empty() {
            return Ok(entries);
        }
        if let Some(row) = self.index.find(&word) {
            entries.push(self.entry(row, Found::Exact, &WordClass::ALL)?);
        }
        for (base, classes) in self.base_forms(&word) {
            if let Some(row) = self.index.find(&base) {
                entries.push(self.entry(row, Found::BaseForm, &classes)?);
            }
        }
        Ok(entries)
    }

    /// Gives at most `limit` words that start with the typed text: the text
    /// itself first if it is a word, then the more frequent words, then the
    /// others in alphabetical order. A space at the end of the text asks for
    /// the collocations that start with the text.
    pub fn suggestions(&self, text: &str, limit: usize) -> Vec<String> {
        let mut prefix = index_form(text);
        if prefix.is_empty() || limit == 0 {
            return Vec::new();
        }
        if text.ends_with(char::is_whitespace) {
            prefix.push('_');
        }
        let rank = |row: &usize| {
            let typed_word = self.index.lemma(*row) == prefix;
            (!typed_word, Reverse(self.index.frequency(*row)), *row)
        };
        let mut rows: Vec<usize> = self.index.with_prefix(&prefix).collect();
        if rows.len() > limit {
            rows.select_nth_unstable_by_key(limit - 1, rank);
            rows.truncate(limit);
        }
        rows.sort_unstable_by_key(rank);
        rows.into_iter()
            .map(|row| display_form(self.index.lemma(row)))
            .collect()
    }

    /// The base forms of `word` and the classes in which each is a base form.
    fn base_forms(&self, word: &str) -> Vec<(String, Vec<WordClass>)> {
        let mut found: Vec<(String, Vec<WordClass>)> = Vec::new();
        for class in WordClass::ALL {
            let is_word = |form: &str| self.index.contains(form, class);
            for base in morph::base_forms(word, class, &self.exceptions, &is_word) {
                match found.iter_mut().find(|(known, _)| *known == base) {
                    Some((_, classes)) => classes.push(class),
                    None => found.push((base, vec![class])),
                }
            }
        }
        found
    }

    fn entry(&self, row: usize, found: Found, classes: &[WordClass]) -> Result<Entry, Error> {
        let lemma = self.index.lemma(row);
        let mut classes = classes.to_vec();
        // A stable sort keeps the order noun, verb, adjective, adverb for equal counts.
        classes.sort_by_key(|&class| Reverse(self.index.tagged(row, class)));
        let mut meanings = Vec::new();
        for class in classes {
            for &offset in self.index.senses(row, class) {
                meanings.push(self.meaning(class, offset, lemma)?);
            }
        }
        Ok(Entry {
            word: display_form(lemma),
            found,
            meanings,
        })
    }

    fn meaning(&self, class: WordClass, offset: u32, lemma: &str) -> Result<Meaning, Error> {
        let synset = self.data[class as usize].synset(offset)?;
        let (definition, examples) = gloss::split(&synset.gloss);
        let is_lemma = |word: &String| word.to_lowercase() == lemma;
        // Word numbers in pointers start at 1.
        let lemma_number = synset.words.iter().position(is_lemma).map(|i| i + 1);
        let mut meaning = Meaning {
            word_class: class,
            definition,
            examples,
            synonyms: Vec::new(),
            antonyms: Vec::new(),
            see_also: Vec::new(),
            hypernyms: Vec::new(),
            hyponyms: Vec::new(),
        };
        for word in synset.words.iter().filter(|word| !is_lemma(word)) {
            push_new(&mut meaning.synonyms, display_form(word));
        }
        for pointer in &synset.pointers {
            let list = match pointer.symbol.as_str() {
                "!" => &mut meaning.antonyms,
                "^" => &mut meaning.see_also,
                "@" | "@i" => &mut meaning.hypernyms,
                "~" | "~i" => &mut meaning.hyponyms,
                _ => continue,
            };
            if pointer.source != 0 && Some(pointer.source) != lemma_number {
                continue;
            }
            let target = self.data[pointer.class as usize].synset(pointer.offset)?;
            let words = match pointer.target {
                0 => &target.words[..],
                number => target.words.get(number - 1..number).unwrap_or_default(),
            };
            for word in words {
                push_new(list, display_form(word));
            }
        }
        Ok(meaning)
    }
}

/// The form of the index files: lower case, with `_` between the words.
fn index_form(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .to_lowercase()
}

/// The form for the user interface: a space between the words.
fn display_form(word: &str) -> String {
    word.replace('_', " ")
}

fn push_new(list: &mut Vec<String>, word: String) {
    if !list.contains(&word) {
        list.push(word);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs only when `PETLAUNCHER_WORDNET_PATH` gives the real WordNet data.
    /// A synonym is a link in the dictionary window, so each word of each
    /// synset must be a word of the index of its class.
    #[test]
    fn real_every_synset_word_is_a_word_of_its_class() {
        let Some(path) = std::env::var_os("PETLAUNCHER_WORDNET_PATH") else {
            return;
        };
        let dictionary = Dictionary::open(&path).expect("the real WordNet data");
        let mut checked = 0;
        for class in WordClass::ALL {
            let file = Path::new(&path).join(format!("data.{}", class.file_suffix()));
            let text = index::read_text(&file).expect("a data file");
            let mut offset = 0;
            for line in text.split_inclusive('\n') {
                if !line.starts_with("  ") {
                    let synset = data::parse_synset(line, offset).expect("a synset");
                    for word in synset.words {
                        let lemma = word.to_lowercase();
                        assert!(dictionary.index.contains(&lemma, class), "{lemma} {class}");
                        checked += 1;
                    }
                }
                offset += line.len() as u32;
            }
        }
        assert!(checked > 200_000, "{checked}");
    }
}
