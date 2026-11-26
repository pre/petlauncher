//! The stored index: one file in the cache directory.
//!
//! `docs/file-search.md` gives the format. [`load`] does not trust a file:
//! a file with a different format version, a different scope, or with damage
//! gives an error, and the next scan replaces it.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::index::{fold_into, Entry, Index};
use crate::scope::Scope;

/// The name of the file in the cache directory.
pub const FILE_NAME: &str = "file-index.bin";

/// Change this number when the format changes.
pub const FORMAT_VERSION: u32 = 1;

const MAGIC: &[u8; 8] = b"PLINDEX\0";
const HEADER_LEN: usize = 16;
const ENTRY_LEN: usize = 13;

#[derive(Debug)]
pub enum LoadError {
    Missing,
    Io(io::Error),
    /// The file has this other format version.
    Version(u32),
    /// The file is for other roots, other ignore entries or another home.
    OtherScope,
    Damaged,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("there is no stored file index"),
            Self::Io(error) => write!(f, "cannot read the stored file index: {error}"),
            Self::Version(version) => write!(
                f,
                "the stored file index has the format version {version}, not {FORMAT_VERSION}"
            ),
            Self::OtherScope => {
                f.write_str("the stored file index has other directories or ignore entries")
            }
            Self::Damaged => f.write_str("the stored file index is damaged"),
        }
    }
}

impl std::error::Error for LoadError {}

/// Writes the index to `file` atomically: first to a temporary file in the
/// same directory, then a rename. Makes the directory if necessary.
pub fn save(file: &Path, scope: &Scope, index: &Index) -> io::Result<()> {
    let dir = file.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let mut temporary = tempfile::NamedTempFile::new_in(dir)?;
    temporary.write_all(&encode(scope, index))?;
    temporary.as_file().sync_all()?;
    temporary.persist(file).map_err(|error| error.error)?;
    Ok(())
}

/// Reads the index from `file` when the file is valid for `scope`.
pub fn load(file: &Path, scope: &Scope) -> Result<Index, LoadError> {
    match fs::read(file) {
        Ok(bytes) => decode(&bytes, scope),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(LoadError::Missing),
        Err(error) => Err(LoadError::Io(error)),
    }
}

fn encode(scope: &Scope, index: &Index) -> Vec<u8> {
    let mut body = Vec::with_capacity(
        index.dir_paths.len()
            + index.dir_ends.len() * 4
            + index.names.len()
            + index.entries.len() * ENTRY_LEN
            + 1024,
    );
    put_scope(&mut body, scope);
    put_u64(&mut body, nanoseconds(index.scanned_at));
    put_u32(&mut body, index.errors);
    put_bytes(&mut body, &index.dir_paths);
    put_u32(&mut body, length(index.dir_ends.len()));
    for end in &index.dir_ends {
        put_u32(&mut body, *end);
    }
    put_bytes(&mut body, &index.names);
    put_u32(&mut body, length(index.entries.len()));
    for entry in &index.entries {
        put_u32(&mut body, entry.dir);
        put_u32(&mut body, entry.name_end);
        put_u32(&mut body, entry.modified);
        body.push(u8::from(entry.is_dir));
    }

    let mut file = Vec::with_capacity(HEADER_LEN + body.len());
    file.extend_from_slice(MAGIC);
    put_u32(&mut file, FORMAT_VERSION);
    put_u32(&mut file, crc32fast::hash(&body));
    file.extend_from_slice(&body);
    file
}

fn decode(file: &[u8], scope: &Scope) -> Result<Index, LoadError> {
    if file.len() < HEADER_LEN || &file[..8] != MAGIC {
        return Err(LoadError::Damaged);
    }
    let mut header = Reader(&file[8..HEADER_LEN]);
    let version = header.u32()?;
    if version != FORMAT_VERSION {
        return Err(LoadError::Version(version));
    }
    let checksum = header.u32()?;
    let body = &file[HEADER_LEN..];
    if crc32fast::hash(body) != checksum {
        return Err(LoadError::Damaged);
    }
    let mut expected_scope = Vec::new();
    put_scope(&mut expected_scope, scope);
    let Some(rest) = body.strip_prefix(expected_scope.as_slice()) else {
        return Err(LoadError::OtherScope);
    };

    let mut reader = Reader(rest);
    let scanned_at = match reader.u64()? {
        0 => None,
        nanoseconds => Some(UNIX_EPOCH + Duration::from_nanos(nanoseconds)),
    };
    let errors = reader.u32()?;
    let dir_paths = reader.bytes()?.to_vec();
    let dir_count = reader.u32()? as usize;
    let dir_ends = (0..dir_count)
        .map(|_| reader.u32())
        .collect::<Result<Vec<_>, _>>()?;
    let names = reader.bytes()?.to_vec();
    let entry_count = reader.u32()? as usize;
    let entries = (0..entry_count)
        .map(|_| {
            Ok(Entry {
                dir: reader.u32()?,
                name_end: reader.u32()?,
                modified: reader.u32()?,
                is_dir: match reader.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(LoadError::Damaged),
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let valid = reader.0.is_empty()
        && ends_are_valid(dir_ends.iter().copied(), dir_paths.len())
        && ends_are_valid(entries.iter().map(|e| e.name_end), names.len())
        && entries.iter().all(|e| (e.dir as usize) < dir_ends.len());
    if !valid {
        return Err(LoadError::Damaged);
    }

    let mut folded = Vec::with_capacity(names.len());
    let mut start = 0;
    for entry in &entries {
        let end = entry.name_end as usize;
        fold_into(&names[start..end], &mut folded);
        start = end;
    }
    Ok(Index {
        scanned_at,
        errors,
        dir_paths,
        dir_ends,
        names,
        folded,
        entries,
    })
}

/// True when each end is after the end before it, and the last end is the
/// length of the buffer. Then no slice is empty or outside the buffer.
fn ends_are_valid(ends: impl Iterator<Item = u32>, buffer_len: usize) -> bool {
    let mut previous = 0;
    for end in ends {
        if end as usize <= previous {
            return false;
        }
        previous = end as usize;
    }
    previous == buffer_len
}

fn put_scope(out: &mut Vec<u8>, scope: &Scope) {
    put_bytes(out, scope.home.as_os_str().as_bytes());
    out.push(u8::from(scope.outside_ignore_files));
    for list in [&scope.roots, &scope.ignore] {
        put_u32(out, length(list.len()));
        for text in list {
            put_bytes(out, text.as_bytes());
        }
    }
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    put_u32(out, length(bytes.len()));
    out.extend_from_slice(bytes);
}

fn length(len: usize) -> u32 {
    u32::try_from(len).expect("the index is larger than 4 GB")
}

fn nanoseconds(time: Option<SystemTime>) -> u64 {
    time.and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], LoadError> {
        if self.0.len() < len {
            return Err(LoadError::Damaged);
        }
        let (taken, rest) = self.0.split_at(len);
        self.0 = rest;
        Ok(taken)
    }

    fn u8(&mut self) -> Result<u8, LoadError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, LoadError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> Result<u64, LoadError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn bytes(&mut self) -> Result<&'a [u8], LoadError> {
        let len = self.u32()? as usize;
        self.take(len)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::index::FileEntry;

    fn scope() -> Scope {
        Scope::new(vec!["~".into()], vec![], PathBuf::from("/h"))
    }

    fn index() -> Index {
        let entry = |path: &str| FileEntry {
            path: PathBuf::from(path),
            is_dir: false,
            modified: UNIX_EPOCH,
        };
        Index::from_entries(vec![entry("/h/a/1"), entry("/h/b/2")], UNIX_EPOCH)
    }

    /// A file with a correct checksum but wrong contents, as a bug or a hand
    /// edit can make it.
    fn decode_changed(change: impl FnOnce(&mut Index)) -> Result<Index, LoadError> {
        let mut index = index();
        change(&mut index);
        decode(&encode(&scope(), &index), &scope())
    }

    #[test]
    fn accepts_the_index_without_a_change() {
        assert_eq!(decode_changed(|_| {}).unwrap().len(), 2);
    }

    #[test]
    fn rejects_a_name_outside_the_names() {
        let result = decode_changed(|index| index.entries[1].name_end += 1);

        assert!(matches!(result, Err(LoadError::Damaged)));
    }

    #[test]
    fn rejects_an_empty_name() {
        let result = decode_changed(|index| index.entries[0].name_end = 0);

        assert!(matches!(result, Err(LoadError::Damaged)));
    }

    #[test]
    fn rejects_a_directory_number_outside_the_directories() {
        let result = decode_changed(|index| index.entries[0].dir = 2);

        assert!(matches!(result, Err(LoadError::Damaged)));
    }

    #[test]
    fn rejects_a_directory_end_outside_the_paths() {
        let result = decode_changed(|index| index.dir_ends[1] += 1);

        assert!(matches!(result, Err(LoadError::Damaged)));
    }

    #[test]
    fn rejects_a_kind_that_is_not_0_or_1() {
        let mut file = encode(&scope(), &index());
        let last = file.len() - 1;
        file[last] = 2;
        let checksum = crc32fast::hash(&file[HEADER_LEN..]);
        file[12..16].copy_from_slice(&checksum.to_le_bytes());

        assert!(matches!(decode(&file, &scope()), Err(LoadError::Damaged)));
    }

    #[test]
    fn keeps_an_index_that_no_scan_made_as_not_scanned() {
        let file = encode(&scope(), &Index::default());

        assert_eq!(decode(&file, &scope()).unwrap().scanned_at(), None);
    }
}
