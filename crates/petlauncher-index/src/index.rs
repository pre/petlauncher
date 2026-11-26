//! The index in memory.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::mem::size_of;
use std::ops::Range;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A file or a directory of the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    /// True for a directory. A symbolic link is not a directory.
    pub is_dir: bool,
    /// The time of the last change, in whole seconds.
    pub modified: SystemTime,
}

/// An immutable snapshot of the file index.
///
/// The index stores each directory path one time. Each entry has the number
/// of its directory and its name. All names are in one byte buffer, and a
/// second buffer has the same names in lower case for the search.
#[derive(Debug, Clone, Default)]
pub struct Index {
    pub(crate) scanned_at: Option<SystemTime>,
    pub(crate) errors: u32,
    pub(crate) dir_paths: Vec<u8>,
    pub(crate) dir_ends: Vec<u32>,
    pub(crate) names: Vec<u8>,
    pub(crate) folded: Vec<u8>,
    pub(crate) entries: Vec<Entry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Entry {
    pub dir: u32,
    /// The end of the name in `names` and `folded`. The name starts at the
    /// end of the name of the entry before.
    pub name_end: u32,
    /// Seconds after the Unix epoch.
    pub modified: u32,
    pub is_dir: bool,
}

impl Index {
    /// Makes an index from a list of entries. Each path is absolute, with no
    /// slash at the end, as a scan gives it. Other paths are left out. Two
    /// entries with the same path give one entry.
    pub fn from_entries(entries: Vec<FileEntry>, scanned_at: SystemTime) -> Self {
        Self::build(entries, scanned_at, 0)
    }

    pub(crate) fn build(mut entries: Vec<FileEntry>, scanned_at: SystemTime, errors: u32) -> Self {
        // Byte operations, because the operations of `Path` parse the
        // components and take most of the time of a build.
        entries.sort_unstable_by(|a, b| bytes(&a.path).cmp(bytes(&b.path)));
        entries.dedup_by(|a, b| bytes(&a.path) == bytes(&b.path));

        let mut index = Self {
            scanned_at: Some(scanned_at),
            errors,
            ..Self::default()
        };
        let mut dir_numbers: HashMap<&[u8], u32> = HashMap::new();
        let mut last_dir: Option<(&[u8], u32)> = None;
        for entry in &entries {
            let path = bytes(&entry.path);
            let Some(slash) = memchr::memrchr(b'/', path) else {
                continue;
            };
            let (parent, name) = match (&path[..slash], &path[slash + 1..]) {
                (_, []) => continue,
                ([], name) => (&b"/"[..], name),
                parent_and_name => parent_and_name,
            };
            let dir = match last_dir {
                Some((path, number)) if path == parent => number,
                _ => {
                    let number = *dir_numbers
                        .entry(parent)
                        .or_insert_with(|| index.push_dir(parent));
                    last_dir = Some((parent, number));
                    number
                }
            };
            index.push_entry(dir, name, entry.is_dir, seconds(entry.modified));
        }
        index.shrink_to_fit();
        index
    }

    fn push_dir(&mut self, path: &[u8]) -> u32 {
        self.dir_paths.extend_from_slice(path);
        self.dir_ends.push(offset(self.dir_paths.len()));
        offset(self.dir_ends.len() - 1)
    }

    fn push_entry(&mut self, dir: u32, name: &[u8], is_dir: bool, modified: u32) {
        self.names.extend_from_slice(name);
        fold_into(name, &mut self.folded);
        self.entries.push(Entry {
            dir,
            name_end: offset(self.names.len()),
            modified,
            is_dir,
        });
    }

    fn shrink_to_fit(&mut self) {
        self.dir_paths.shrink_to_fit();
        self.dir_ends.shrink_to_fit();
        self.names.shrink_to_fit();
        self.folded.shrink_to_fit();
        self.entries.shrink_to_fit();
    }

    /// The number of files and directories.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The start time of the scan that made the index. `None` for an empty
    /// index that no scan made.
    pub fn scanned_at(&self) -> Option<SystemTime> {
        self.scanned_at
    }

    /// The number of directories, entries and ignore files that the scan
    /// could not read.
    pub fn errors(&self) -> usize {
        self.errors as usize
    }

    /// The bytes that the index uses on the heap.
    pub fn heap_size(&self) -> usize {
        self.dir_paths.capacity()
            + self.dir_ends.capacity() * size_of::<u32>()
            + self.names.capacity()
            + self.folded.capacity()
            + self.entries.capacity() * size_of::<Entry>()
    }

    /// All entries, sorted by path.
    pub fn iter(&self) -> impl Iterator<Item = FileEntry> + '_ {
        (0..self.entries.len()).map(|i| self.entry(i))
    }

    pub(crate) fn entry(&self, i: usize) -> FileEntry {
        let entry = &self.entries[i];
        let path = self.path_parts(i).concat();
        FileEntry {
            path: PathBuf::from(OsStr::from_bytes(&path)),
            is_dir: entry.is_dir,
            modified: UNIX_EPOCH + Duration::from_secs(entry.modified.into()),
        }
    }

    /// The directory, the separator and the name that make the path of an
    /// entry. The separator is empty in the directory `/`.
    pub(crate) fn path_parts(&self, i: usize) -> [&[u8]; 3] {
        let dir = self.dir_path(self.entries[i].dir);
        let separator: &[u8] = if dir.ends_with(b"/") { b"" } else { b"/" };
        [dir, separator, &self.names[self.name_range(i)]]
    }

    pub(crate) fn dir_path(&self, dir: u32) -> &[u8] {
        let dir = dir as usize;
        let start = if dir == 0 { 0 } else { self.dir_ends[dir - 1] };
        &self.dir_paths[start as usize..self.dir_ends[dir] as usize]
    }

    pub(crate) fn name_range(&self, i: usize) -> Range<usize> {
        let start = if i == 0 {
            0
        } else {
            self.entries[i - 1].name_end
        };
        start as usize..self.entries[i].name_end as usize
    }
}

fn bytes(path: &std::path::Path) -> &[u8] {
    path.as_os_str().as_bytes()
}

fn offset(value: usize) -> u32 {
    u32::try_from(value).expect("the index is larger than 4 GB")
}

fn seconds(time: SystemTime) -> u32 {
    let seconds = time.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    u32::try_from(seconds).unwrap_or(u32::MAX)
}

/// Writes the lower case of `name` to `out`, with the same byte length.
///
/// The byte positions of a name and of its lower case are then the same, so
/// one offset list serves the two buffers. A letter whose lower case has a
/// different byte length stays as it is. A name that is not UTF-8 gets only
/// the ASCII lower case.
pub(crate) fn fold_into(name: &[u8], out: &mut Vec<u8>) {
    match std::str::from_utf8(name) {
        Ok(text) if !text.is_ascii() => {
            let mut buffer = [0; 4];
            for c in text.chars() {
                out.extend_from_slice(fold_char(c).encode_utf8(&mut buffer).as_bytes());
            }
        }
        _ => out.extend(name.iter().map(u8::to_ascii_lowercase)),
    }
}

fn fold_char(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(l), None) if l.len_utf8() == c.len_utf8() => l,
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fold(text: &str) -> Vec<u8> {
        let mut out = Vec::new();
        fold_into(text.as_bytes(), &mut out);
        out
    }

    #[test]
    fn folds_to_lower_case_with_the_same_byte_length() {
        assert_eq!(fold("README.md"), b"readme.md");
        assert_eq!(fold("ÄÖ-Ångström"), "äö-ångström".as_bytes());
        // U+0130 has a lower case of two characters, so it stays.
        assert_eq!(fold("İx"), "İx".as_bytes());
    }

    #[test]
    fn folds_only_ascii_in_a_name_that_is_not_utf8() {
        let mut out = Vec::new();

        fold_into(b"AB\xffC", &mut out);

        assert_eq!(out, b"ab\xffc");
    }

    #[test]
    fn stores_each_directory_one_time() {
        let at = |path: &str| FileEntry {
            path: PathBuf::from(path),
            is_dir: false,
            modified: UNIX_EPOCH,
        };

        let index = Index::from_entries(
            vec![at("/h/a/2"), at("/h/b/1"), at("/h/a/1"), at("/h/a/1")],
            UNIX_EPOCH,
        );

        assert_eq!(index.len(), 3);
        assert_eq!(index.dir_ends.len(), 2);
        assert_eq!(index.dir_paths, b"/h/a/h/b");
        let paths: Vec<_> = index.iter().map(|e| e.path).collect();
        assert_eq!(paths, ["/h/a/1", "/h/a/2", "/h/b/1"].map(PathBuf::from));
    }

    #[test]
    fn leaves_out_a_path_with_a_slash_at_the_end_or_with_no_slash() {
        let at = |path: &str| FileEntry {
            path: PathBuf::from(path),
            is_dir: false,
            modified: UNIX_EPOCH,
        };

        let index = Index::from_entries(vec![at("/h/a/"), at("name"), at("/h/b")], UNIX_EPOCH);

        let paths: Vec<_> = index.iter().map(|e| e.path).collect();
        assert_eq!(paths, [PathBuf::from("/h/b")]);
    }

    #[test]
    fn adds_no_second_slash_below_the_root_directory() {
        let index = Index::from_entries(
            vec![FileEntry {
                path: PathBuf::from("/data"),
                is_dir: true,
                modified: UNIX_EPOCH,
            }],
            UNIX_EPOCH,
        );

        assert_eq!(index.iter().next().unwrap().path.as_os_str(), "/data");
    }

    #[test]
    fn keeps_the_kind_and_the_time_in_seconds() {
        let modified = UNIX_EPOCH + Duration::from_millis(1_700_000_000_900);

        let index = Index::from_entries(
            vec![FileEntry {
                path: PathBuf::from("/h/dir"),
                is_dir: true,
                modified,
            }],
            UNIX_EPOCH,
        );

        let entry = index.iter().next().unwrap();
        assert!(entry.is_dir);
        assert_eq!(
            entry.modified,
            UNIX_EPOCH + Duration::from_secs(1_700_000_000)
        );
        assert!(index.heap_size() > 0);
    }
}
