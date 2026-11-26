//! The index files, merged into one sorted table of all words in memory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::{Error, WordClass};

/// All words of the four index files. A word that is in two or more word
/// classes has one row.
pub(crate) struct Index {
    /// The words, one after the other, in the order of `words`.
    text: String,
    words: Vec<Word>,
    /// The synset offsets of all words, word by word and class by class.
    offsets: Vec<u32>,
}

/// One row of the index. The fields are small numbers to keep the table small.
#[derive(Clone, Copy)]
struct Word {
    text_start: u32,
    text_len: u16,
    first_offset: u32,
    frequency: u32,
    sense_counts: [u8; 4],
    tagged_counts: [u8; 4],
}

/// One line of an index file.
#[derive(Debug, PartialEq, Eq)]
struct IndexLine<'a> {
    lemma: &'a str,
    tagged: u8,
    offsets: Vec<u32>,
}

impl Index {
    /// Reads the four index files and the optional frequency file `cntlist.rev`.
    pub(crate) fn read(dir: &Path) -> Result<Index, Error> {
        let mut files = Vec::with_capacity(4);
        for class in WordClass::ALL {
            let path = dir.join(format!("index.{}", class.file_suffix()));
            let text = read_text(&path)?;
            files.push((class, path, text));
        }
        let frequencies_path = dir.join("cntlist.rev");
        let frequencies_text = match std::fs::read_to_string(&frequencies_path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(source) => {
                return Err(Error::Read {
                    path: frequencies_path,
                    source,
                })
            }
        };
        Index::build(&files, &parse_frequencies(&frequencies_text))
    }

    /// Merges the lines of the files into one table that is sorted by word.
    ///
    /// The lines are sorted before they are parsed, so that the temporary
    /// list of offsets of a line is free before the next line. Thousands of
    /// small allocations that live at the same time would stay in the
    /// resident memory of the process after the build.
    fn build(
        files: &[(WordClass, PathBuf, String)],
        frequencies: &HashMap<&str, u32>,
    ) -> Result<Index, Error> {
        let mut lines = Vec::new();
        for (class, path, text) in files {
            for (number, line) in text.lines().enumerate() {
                if line.starts_with("  ") || line.trim().is_empty() {
                    continue;
                }
                let lemma = line.split(' ').next().unwrap_or_default();
                lines.push((lemma, *class, path, number + 1, line));
            }
        }
        // A stable sort keeps the class order of the files for each word.
        lines.sort_by(|a, b| a.0.cmp(b.0));

        let mut index = Index {
            text: String::with_capacity(lines.iter().map(|line| line.0.len()).sum()),
            words: Vec::with_capacity(lines.len()),
            offsets: Vec::with_capacity(lines.len() * 2),
        };
        for (_, class, path, number, line) in lines {
            let line = parse_line(line).ok_or_else(|| Error::Format {
                path: path.clone(),
                detail: format!("line {number} is not an index entry"),
            })?;
            let is_new_word = index
                .words
                .last()
                .is_none_or(|word| index.text_of(word) != line.lemma);
            if is_new_word {
                index.words.push(Word {
                    text_start: index.text.len() as u32,
                    text_len: line.lemma.len() as u16,
                    first_offset: index.offsets.len() as u32,
                    frequency: frequencies.get(line.lemma).copied().unwrap_or(0),
                    sense_counts: [0; 4],
                    tagged_counts: [0; 4],
                });
                index.text.push_str(line.lemma);
            }
            let word = index.words.last_mut().expect("a word");
            if word.sense_counts[class as usize] != 0 {
                return Err(Error::Format {
                    path: path.clone(),
                    detail: format!("the word {} is in the file two times", line.lemma),
                });
            }
            word.sense_counts[class as usize] = line.offsets.len() as u8;
            word.tagged_counts[class as usize] = line.tagged;
            index.offsets.extend(line.offsets);
        }
        index.text.shrink_to_fit();
        index.words.shrink_to_fit();
        index.offsets.shrink_to_fit();
        Ok(index)
    }

    /// The number of words.
    pub(crate) fn len(&self) -> usize {
        self.words.len()
    }

    /// The row of a word in the index form: lower case, `_` between the parts.
    pub(crate) fn find(&self, lemma: &str) -> Option<usize> {
        self.words
            .binary_search_by(|word| self.text_of(word).cmp(lemma))
            .ok()
    }

    pub(crate) fn contains(&self, lemma: &str, class: WordClass) -> bool {
        self.find(lemma)
            .is_some_and(|row| !self.senses(row, class).is_empty())
    }

    pub(crate) fn lemma(&self, row: usize) -> &str {
        self.text_of(&self.words[row])
    }

    /// The synset offsets of a word in one class, the most frequent sense first.
    pub(crate) fn senses(&self, row: usize, class: WordClass) -> &[u32] {
        let word = &self.words[row];
        let counts = word.sense_counts.map(usize::from);
        let start = word.first_offset as usize + counts[..class as usize].iter().sum::<usize>();
        &self.offsets[start..start + counts[class as usize]]
    }

    /// The number of senses of a word in one class that the semantic
    /// concordance texts tag.
    pub(crate) fn tagged(&self, row: usize, class: WordClass) -> u8 {
        self.words[row].tagged_counts[class as usize]
    }

    /// How many times the semantic concordance texts tag the word.
    pub(crate) fn frequency(&self, row: usize) -> u32 {
        self.words[row].frequency
    }

    /// The rows of the words that start with `prefix`, in alphabetical order.
    pub(crate) fn with_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = usize> + 'a {
        let start = self
            .words
            .partition_point(|word| self.text_of(word) < prefix);
        (start..self.words.len()).take_while(move |&row| self.lemma(row).starts_with(prefix))
    }

    fn text_of(&self, word: &Word) -> &str {
        let start = word.text_start as usize;
        &self.text[start..start + usize::from(word.text_len)]
    }
}

pub(crate) fn read_text(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })
}

/// Parses `lemma pos synset_cnt p_cnt [ptr_symbol...] sense_cnt tagsense_cnt
/// synset_offset...`.
fn parse_line(line: &str) -> Option<IndexLine<'_>> {
    let mut fields = line.split_ascii_whitespace();
    let lemma = fields.next()?;
    let _pos = fields.next()?;
    let synset_count: u8 = fields.next()?.parse().ok()?;
    let pointer_count: usize = fields.next()?.parse().ok()?;
    let mut fields = fields.skip(pointer_count);
    let _sense_count = fields.next()?;
    let tagged = fields.next()?.parse().ok()?;
    let offsets: Vec<u32> = fields
        .map(|field| field.parse().ok())
        .collect::<Option<_>>()?;
    let fits = lemma.len() <= usize::from(u16::MAX) && offsets.len() == usize::from(synset_count);
    fits.then_some(IndexLine {
        lemma,
        tagged,
        offsets,
    })
}

/// Sums the tag counts of `cntlist.rev` for each word. A line is
/// `sense_key sense_number tag_cnt`, and the sense key starts with `lemma%`.
/// The order of the suggestions is the only use of this file, so the reader
/// skips a line that it cannot parse.
fn parse_frequencies(text: &str) -> HashMap<&str, u32> {
    let mut frequencies: HashMap<&str, u32> = HashMap::new();
    for line in text.lines() {
        let mut fields = line.split_ascii_whitespace();
        let (Some(key), Some(_), Some(count)) = (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let (Some((lemma, _)), Ok(count)) = (key.split_once('%'), count.parse::<u32>()) else {
            continue;
        };
        *frequencies.entry(lemma).or_default() += count;
    }
    frequencies
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(noun: &str, verb: &str, frequencies: &str) -> Index {
        let files = [
            (
                WordClass::Noun,
                PathBuf::from("index.noun"),
                noun.to_owned(),
            ),
            (
                WordClass::Verb,
                PathBuf::from("index.verb"),
                verb.to_owned(),
            ),
            (
                WordClass::Adjective,
                PathBuf::from("index.adj"),
                String::new(),
            ),
            (WordClass::Adverb, PathBuf::from("index.adv"), String::new()),
        ];
        Index::build(&files, &parse_frequencies(frequencies)).expect("an index")
    }

    #[test]
    fn index_line_gives_lemma_tagged_count_and_offsets() {
        let line = "launcher n 2 3 @ ~ + 2 1 03647520 02982232  ";

        let parsed = parse_line(line);

        assert_eq!(
            parsed,
            Some(IndexLine {
                lemma: "launcher",
                tagged: 1,
                offsets: vec![3647520, 2982232]
            })
        );
    }

    #[test]
    fn index_line_with_missing_offsets_is_not_an_entry() {
        assert_eq!(parse_line("launcher n 2 0 2 1 03647520  "), None);
        assert_eq!(parse_line("launcher n x 0 1 1 03647520"), None);
        assert_eq!(parse_line("launcher"), None);
    }

    #[test]
    fn word_in_two_classes_has_one_row_with_both_classes() {
        let index = index(
            "launch n 1 0 1 0 00000010  \n",
            "launch v 2 0 2 2 00000020 00000030  \n",
            "",
        );

        let row = index.find("launch").expect("a row");

        assert_eq!(index.lemma(row), "launch");
        assert_eq!(index.senses(row, WordClass::Noun), [10]);
        assert_eq!(index.senses(row, WordClass::Verb), [20, 30]);
        assert!(index.senses(row, WordClass::Adjective).is_empty());
        assert_eq!(index.tagged(row, WordClass::Verb), 2);
        assert!(index.contains("launch", WordClass::Verb));
        assert!(!index.contains("launch", WordClass::Adverb));
        assert!(!index.contains("lunch", WordClass::Noun));
    }

    #[test]
    fn header_lines_with_two_spaces_are_skipped() {
        let index = index("  1 a license text  \nbox n 1 0 1 0 00000010  \n", "", "");

        assert_eq!(index.find("box").map(|row| index.lemma(row)), Some("box"));
        assert_eq!(index.find("  1"), None);
    }

    #[test]
    fn prefix_gives_the_words_that_start_with_it_in_alphabetical_order() {
        let index = index(
            "launch n 1 0 1 0 00000010  \nlaunch_pad n 1 0 1 0 00000020  \n\
             launcher n 1 0 1 0 00000030  \nlunch n 1 0 1 0 00000040  \n",
            "laugh v 1 0 1 0 00000010  \nlaunch v 1 0 1 0 00000020  \n",
            "",
        );

        let words: Vec<&str> = index
            .with_prefix("launch")
            .map(|row| index.lemma(row))
            .collect();

        assert_eq!(words, ["launch", "launch_pad", "launcher"]);
    }

    #[test]
    fn frequency_is_the_sum_of_the_tag_counts_of_the_word() {
        let index = index(
            "launch n 1 0 1 0 00000010  \nlauncher n 1 0 1 0 00000020  \n",
            "",
            "launch%2:35:00:: 1 20\nlaunch%1:06:00:: 1 3\nbad line\n",
        );

        assert_eq!(index.frequency(index.find("launch").unwrap()), 23);
        assert_eq!(index.frequency(index.find("launcher").unwrap()), 0);
    }

    #[test]
    fn bad_index_line_is_a_format_error_with_its_line_number() {
        let files = [(
            WordClass::Noun,
            PathBuf::from("index.noun"),
            "  1 header\nbox n 1 0 1 0 00000010\nbad\n".to_owned(),
        )];

        let error = Index::build(&files, &HashMap::new())
            .err()
            .expect("an error");

        assert!(error.to_string().contains("line 3"), "{error}");
    }

    #[test]
    fn same_word_twice_in_one_file_is_a_format_error() {
        let files = [(
            WordClass::Noun,
            PathBuf::from("index.noun"),
            "box n 1 0 1 0 00000010\nbox n 1 0 1 0 00000020\n".to_owned(),
        )];

        assert!(Index::build(&files, &HashMap::new()).is_err());
    }
}
