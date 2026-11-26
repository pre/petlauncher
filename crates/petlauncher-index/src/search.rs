//! The search by file name. `docs/file-search.md` gives the match and rank
//! rule.

use std::cmp::Ordering;

use memchr::memmem::Finder;

use crate::index::{fold_into, FileEntry, Index};

impl Index {
    /// The best `limit` files and directories whose name matches `text`, best
    /// first.
    pub fn search(&self, text: &str, limit: usize) -> Vec<FileEntry> {
        self.search_kinds(text, limit, true)
    }

    /// Same as [`Index::search`], but without directories.
    pub fn search_files(&self, text: &str, limit: usize) -> Vec<FileEntry> {
        self.search_kinds(text, limit, false)
    }

    fn search_kinds(&self, text: &str, limit: usize, with_dirs: bool) -> Vec<FileEntry> {
        let query = Query::new(text);
        let Some(longest) = query.words.iter().max_by_key(|w| w.needle().len()) else {
            return Vec::new();
        };
        if limit == 0 {
            return Vec::new();
        }
        let haystacks = if query.case_sensitive {
            &self.names
        } else {
            &self.folded
        };

        // One search of the longest word through all names finds the
        // entries that can match. After each found entry, the search
        // continues at the start of the next name. This also applies when
        // the found text starts in one name and ends in the next one.
        let mut found = Vec::new();
        let mut from = 0;
        let mut i = 0;
        while let Some(offset) = longest.find(&haystacks[from..]) {
            let position = from + offset;
            while self.entries[i].name_end as usize <= position {
                i += 1;
            }
            let range = self.name_range(i);
            from = range.end;
            let entry = &self.entries[i];
            if position + longest.needle().len() > range.end || (entry.is_dir && !with_dirs) {
                continue;
            }
            let (haystack, name) = (&haystacks[range.clone()], &self.names[range.clone()]);
            if let Some(score) = query.score(haystack, name) {
                found.push(Candidate {
                    score,
                    name_len: range.len() as u32,
                    modified: entry.modified,
                    index: i as u32,
                });
            }
        }

        let rank = |a: &Candidate, b: &Candidate| self.rank(a, b);
        if found.len() > limit {
            found.select_nth_unstable_by(limit - 1, rank);
            found.truncate(limit);
        }
        found.sort_unstable_by(rank);
        found.iter().map(|c| self.entry(c.index as usize)).collect()
    }

    fn rank(&self, a: &Candidate, b: &Candidate) -> Ordering {
        a.score
            .cmp(&b.score)
            .then(a.name_len.cmp(&b.name_len))
            .then(b.modified.cmp(&a.modified))
            .then_with(|| {
                let a = self.path_parts(a.index as usize);
                let b = self.path_parts(b.index as usize);
                let len = |parts: &[&[u8]; 3]| parts.iter().map(|p| p.len()).sum::<usize>();
                len(&a).cmp(&len(&b)).then_with(|| {
                    a.iter()
                        .flat_map(|p| p.iter())
                        .cmp(b.iter().flat_map(|p| p.iter()))
                })
            })
    }
}

struct Candidate {
    /// The sum of the match classes of the words. Lower is better.
    score: u32,
    name_len: u32,
    modified: u32,
    index: u32,
}

/// How well a word of the text matches a name. Lower is better.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    /// The name is the word.
    Exact,
    /// The name before its last dot is the word.
    Stem,
    /// The name starts with the word.
    Prefix,
    /// A word of the name starts with the word.
    WordStart,
    /// The word is inside the name.
    Inside,
}

struct Query {
    words: Vec<Finder<'static>>,
    case_sensitive: bool,
}

impl Query {
    fn new(text: &str) -> Self {
        let case_sensitive = text.chars().any(char::is_uppercase);
        let words = text
            .split_whitespace()
            .map(|word| {
                let mut needle = Vec::new();
                if case_sensitive {
                    needle.extend_from_slice(word.as_bytes());
                } else {
                    fold_into(word.as_bytes(), &mut needle);
                }
                Finder::new(&needle).into_owned()
            })
            .collect();
        Self {
            words,
            case_sensitive,
        }
    }

    /// The score of a name, or `None` when a word is not in the name.
    /// `haystack` is the name in the case of the search, and `name` is the
    /// name as it is. The two have the same byte positions.
    fn score(&self, haystack: &[u8], name: &[u8]) -> Option<u32> {
        self.words
            .iter()
            .map(|word| classify(word, haystack, name).map(|class| class as u32))
            .sum()
    }
}

fn classify(word: &Finder<'_>, haystack: &[u8], name: &[u8]) -> Option<Class> {
    let len = word.needle().len();
    let mut found = None;
    let mut from = 0;
    // Not `find_iter`, because it skips a match that overlaps the match
    // before it, and that match can start a word.
    while let Some(offset) = word.find(&haystack[from..]) {
        let position = from + offset;
        from = position + 1;
        let class = if position == 0 {
            if len == haystack.len() {
                Class::Exact
            } else if memchr::memrchr(b'.', haystack) == Some(len) {
                Class::Stem
            } else {
                Class::Prefix
            }
        } else if is_word_start(name, position) {
            Class::WordStart
        } else {
            Class::Inside
        };
        // The positions come in order, so only `Inside` can become better.
        if class != Class::Inside {
            return Some(class);
        }
        found = Some(class);
    }
    found
}

/// True after a character that is not a letter or a digit, and at a change
/// from a lower-case to an upper-case ASCII letter, as in `PaletteZoom`.
fn is_word_start(name: &[u8], position: usize) -> bool {
    let before = name[position - 1];
    let here = name[position];
    (before.is_ascii() && !before.is_ascii_alphanumeric())
        || (before.is_ascii_lowercase() && here.is_ascii_uppercase())
}
