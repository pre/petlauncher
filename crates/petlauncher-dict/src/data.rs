//! The data files: one synset on each line, read from the disk when necessary.

use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;

use crate::{Error, WordClass};

/// A synset as the data file gives it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Synset {
    /// The words as in the file, without the adjective markers.
    pub words: Vec<String>,
    pub pointers: Vec<Pointer>,
    pub gloss: String,
}

/// A pointer from a synset, or from one of its words, to a different synset.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Pointer {
    pub symbol: String,
    pub class: WordClass,
    pub offset: u32,
    /// The number of the source word, starting at 1. Zero for a pointer from
    /// the full synset.
    pub source: usize,
    /// The number of the target word, starting at 1. Zero for a pointer to
    /// the full synset.
    pub target: usize,
}

/// An open data file.
pub(crate) struct DataFile {
    path: PathBuf,
    file: File,
}

impl DataFile {
    pub(crate) fn open(path: PathBuf) -> Result<DataFile, Error> {
        match File::open(&path) {
            Ok(file) => Ok(DataFile { path, file }),
            Err(source) => Err(Error::Read { path, source }),
        }
    }

    /// Reads the synset that starts at the byte `offset`.
    pub(crate) fn synset(&self, offset: u32) -> Result<Synset, Error> {
        let line = self.read_line(offset)?;
        parse_synset(&line, offset).ok_or_else(|| Error::Format {
            path: self.path.clone(),
            detail: format!("there is no synset at byte {offset}"),
        })
    }

    fn read_line(&self, offset: u32) -> Result<String, Error> {
        let mut line = Vec::new();
        let mut chunk = [0; 4096];
        let mut position = u64::from(offset);
        loop {
            let count = self
                .file
                .read_at(&mut chunk, position)
                .map_err(|source| Error::Read {
                    path: self.path.clone(),
                    source,
                })?;
            let chunk = &chunk[..count];
            if let Some(end) = chunk.iter().position(|&byte| byte == b'\n') {
                line.extend_from_slice(&chunk[..end]);
                break;
            }
            line.extend_from_slice(chunk);
            if count == 0 {
                break;
            }
            position += count as u64;
        }
        String::from_utf8(line).map_err(|_| Error::Format {
            path: self.path.clone(),
            detail: format!("the synset at byte {offset} is not UTF-8 text"),
        })
    }
}

/// Parses one line of a data file. Gives `None` when the line is not the
/// synset at `offset`.
pub(crate) fn parse_synset(line: &str, offset: u32) -> Option<Synset> {
    let (head, gloss) = line.split_once('|')?;
    let mut fields = head.split_ascii_whitespace();
    if fields.next()?.parse::<u32>().ok()? != offset {
        return None;
    }
    let _lex_filenum = fields.next()?;
    let _synset_type = fields.next()?;
    let word_count = usize::from_str_radix(fields.next()?, 16).ok()?;
    let mut words = Vec::with_capacity(word_count);
    for _ in 0..word_count {
        words.push(without_marker(fields.next()?).to_owned());
        let _lex_id = fields.next()?;
    }
    let pointer_count: usize = fields.next()?.parse().ok()?;
    let mut pointers = Vec::with_capacity(pointer_count);
    for _ in 0..pointer_count {
        let symbol = fields.next()?.to_owned();
        let offset = fields.next()?.parse().ok()?;
        let class = WordClass::from_letter(fields.next()?)?;
        let source_target = fields.next()?;
        if source_target.len() != 4 {
            return None;
        }
        let source = usize::from_str_radix(&source_target[..2], 16).ok()?;
        let target = usize::from_str_radix(&source_target[2..], 16).ok()?;
        pointers.push(Pointer {
            symbol,
            class,
            offset,
            source,
            target,
        });
    }
    Some(Synset {
        words,
        pointers,
        gloss: gloss.trim().to_owned(),
    })
}

/// Removes the syntactic marker of an adjective: `(a)` prenominal,
/// `(p)` predicate or `(ip)` immediately after the noun.
fn without_marker(word: &str) -> &str {
    ["(a)", "(p)", "(ip)"]
        .iter()
        .find_map(|marker| word.strip_suffix(marker))
        .unwrap_or(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOUN: &str = "00000265 06 n 02 launcher 0 rocket_launcher 0 002 \
        @ 00000152 n 0000 + 00000563 v 0101 | a device that fires rockets; \
        \"the soldiers carried a launcher\"  ";

    #[test]
    fn synset_line_gives_words_pointers_and_gloss() {
        let synset = parse_synset(NOUN, 265).expect("a synset");

        assert_eq!(synset.words, ["launcher", "rocket_launcher"]);
        assert_eq!(
            synset.pointers,
            [
                Pointer {
                    symbol: "@".into(),
                    class: WordClass::Noun,
                    offset: 152,
                    source: 0,
                    target: 0
                },
                Pointer {
                    symbol: "+".into(),
                    class: WordClass::Verb,
                    offset: 563,
                    source: 1,
                    target: 1
                },
            ]
        );
        assert_eq!(
            synset.gloss,
            "a device that fires rockets; \"the soldiers carried a launcher\""
        );
    }

    #[test]
    fn adjective_markers_are_not_part_of_the_word() {
        let line = "00000010 00 s 03 galore(ip) 0 afraid(p) 0 former(a) 0 000 | text  ";

        let synset = parse_synset(line, 10).expect("a synset");

        assert_eq!(synset.words, ["galore", "afraid", "former"]);
    }

    #[test]
    fn verb_frames_after_the_pointers_are_ignored() {
        let line = "00000229 38 v 01 run 0 001 @ 00000152 v 0000 \
            02 + 01 00 + 02 00 | move fast  ";

        let synset = parse_synset(line, 229).expect("a synset");

        assert_eq!(synset.words, ["run"]);
        assert_eq!(synset.pointers.len(), 1);
        assert_eq!(synset.gloss, "move fast");
    }

    #[test]
    fn hexadecimal_word_count_and_word_numbers_are_read() {
        let words: String = (0..11).map(|i| format!("w{i} 0 ")).collect();
        let line = format!("00000001 00 v 0b {words}001 ^ 00000002 v 010b | text  ");

        let synset = parse_synset(&line, 1).expect("a synset");

        assert_eq!(synset.words.len(), 11);
        assert_eq!(synset.pointers[0].target, 11);
        assert_eq!(synset.pointers[0].class, WordClass::Verb);
    }

    #[test]
    fn satellite_pointer_class_is_adjective() {
        let line = "00000001 00 a 01 good 0 001 & 00000002 s 0000 | text  ";

        let synset = parse_synset(line, 1).expect("a synset");

        assert_eq!(synset.pointers[0].class, WordClass::Adjective);
    }

    #[test]
    fn line_at_a_different_offset_is_not_a_synset() {
        assert_eq!(parse_synset(NOUN, 266), None);
    }

    #[test]
    fn line_with_too_few_fields_is_not_a_synset() {
        assert_eq!(parse_synset("00000001 06 n 02 launcher 0 | text", 1), None);
        assert_eq!(
            parse_synset("00000001 06 n 01 launcher 0 000 text", 1),
            None
        );
        assert_eq!(
            parse_synset("00000001 06 n 01 x 0 001 @ 1 q 0000 | t", 1),
            None
        );
    }
}
