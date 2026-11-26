//! The path mode: the query is a file path (R1, R10). See `docs/core.md`
//! for the rules.

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cancel::CancelToken;
use crate::contract::{Action, Icon, Mode, Results, Row, RowKind, TypedPath};

/// The largest number of rows in the path mode.
pub(crate) const ROW_LIMIT: usize = 50;

/// When no name contains the typed name, a name must start with at least
/// this number of the same characters to be a near match.
const MIN_SHARED_START: usize = 3;

/// A query that is a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QueryPath {
    /// The absolute path, with no `.` parts and no last slash.
    pub path: PathBuf,
    /// The query started with `~`. The rows then show `~` for the home
    /// directory, and Enter on a directory row puts a query with `~`.
    pub tilde: bool,
}

/// Reads the query as a path, or gives `None` when it is not a path.
///
/// The query is a path when, after the spaces and line ends at its start and
/// end are removed and one pair of quotation marks around it is removed, it
/// starts with `/`, `~/` or `file:///`, or it is `~`.
pub(crate) fn parse(query: &str, home: &Path) -> Option<QueryPath> {
    let text = unquote(query.trim());
    if let Some(rest) = text.strip_prefix("file://") {
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        if !rest.starts_with('/') {
            return None;
        }
        let path = PathBuf::from(OsString::from_vec(percent_decode(rest)));
        return Some(QueryPath {
            path: normalize(&path),
            tilde: false,
        });
    }
    let path = if text == "~" {
        home.to_path_buf()
    } else if let Some(rest) = text.strip_prefix("~/") {
        home.join(rest.trim_start_matches('/'))
    } else if text.starts_with('/') {
        PathBuf::from(text)
    } else {
        return None;
    };
    Some(QueryPath {
        path: normalize(&path),
        tilde: text.starts_with('~'),
    })
}

/// Removes the `.` parts and the last slash.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    path.components().collect()
}

fn unquote(text: &str) -> &str {
    for quote in ['"', '\''] {
        if text.len() >= 2 && text.starts_with(quote) && text.ends_with(quote) {
            return &text[1..text.len() - 1];
        }
    }
    text
}

fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let pair = bytes.get(index + 1..index + 3);
        match (bytes[index], pair.and_then(hex_pair)) {
            (b'%', Some(byte)) => {
                decoded.push(byte);
                index += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    decoded
}

fn hex_pair(pair: &[u8]) -> Option<u8> {
    let text = std::str::from_utf8(pair).ok()?;
    u8::from_str_radix(text, 16).ok()
}

/// Gives the results of the path mode.
pub(crate) fn search(
    query: &str,
    query_path: &QueryPath,
    home: &Path,
    cancel: &CancelToken,
) -> Results {
    let display = Display {
        home,
        tilde: query_path.tilde,
    };
    let path = &query_path.path;
    let (rows, selected, typed_path) = match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => (
            directory_rows(path, &display, cancel),
            None,
            Some(TypedPath::Directory(path.clone())),
        ),
        Ok(_) => (
            vec![entry_row(path, false, &display)],
            Some(0),
            Some(TypedPath::File(path.clone())),
        ),
        Err(_) => {
            let rows = nearest_rows(path, &display, cancel);
            let selected = (!rows.is_empty()).then_some(0);
            (rows, selected, None)
        }
    };
    Results {
        query: query.to_owned(),
        mode: Mode::Path,
        rows,
        selected,
        typed_path,
    }
}

/// The contents of an existing directory: the newest modification first.
fn directory_rows(directory: &Path, display: &Display, cancel: &CancelToken) -> Vec<Row> {
    let mut entries = read_entries(directory, false, cancel);
    entries.sort_by(|a, b| b.modified.cmp(&a.modified).then_with(|| a.compare_names(b)));
    entries
        .iter()
        .take(ROW_LIMIT)
        .map(|entry| entry_row(&entry.path, entry.is_dir, display))
        .collect()
}

/// The matches for the first part of the path that does not exist, in the
/// last folder of the path that exists.
fn nearest_rows(path: &Path, display: &Display, cancel: &CancelToken) -> Vec<Row> {
    let Some((folder, typed_name)) = last_existing_folder(path) else {
        return Vec::new();
    };
    let typed_key = typed_name.to_lowercase();
    let entries = read_entries(&folder, typed_name.starts_with('.'), cancel);
    let mut matches = names_that_contain(&entries, &typed_key);
    if matches.is_empty() {
        matches = names_with_a_shared_start(&entries, &typed_key);
    }
    matches
        .into_iter()
        .take(ROW_LIMIT)
        .map(|entry| entry_row(&entry.path, entry.is_dir, display))
        .collect()
}

fn names_that_contain<'a>(entries: &'a [Entry], typed_key: &str) -> Vec<&'a Entry> {
    let mut ranked: Vec<(u8, &Entry)> = entries
        .iter()
        .filter_map(|entry| contains_rank(&entry.key, typed_key).map(|rank| (rank, entry)))
        .collect();
    ranked.sort_by(|(rank_a, a), (rank_b, b)| rank_a.cmp(rank_b).then_with(|| a.compare_names(b)));
    ranked.into_iter().map(|(_, entry)| entry).collect()
}

fn names_with_a_shared_start<'a>(entries: &'a [Entry], typed_key: &str) -> Vec<&'a Entry> {
    let mut ranked: Vec<(usize, &Entry)> = entries
        .iter()
        .map(|entry| (shared_start(&entry.key, typed_key), entry))
        .filter(|(shared, _)| *shared >= MIN_SHARED_START)
        .collect();
    ranked.sort_by(|(shared_a, a), (shared_b, b)| {
        shared_b.cmp(shared_a).then_with(|| a.compare_names(b))
    });
    ranked.into_iter().map(|(_, entry)| entry).collect()
}

/// 0: the same name, 1: the name starts with the typed name, 2: the name
/// contains it. The two texts are in lower case.
fn contains_rank(name: &str, typed: &str) -> Option<u8> {
    if name == typed {
        Some(0)
    } else if name.starts_with(typed) {
        Some(1)
    } else if name.contains(typed) {
        Some(2)
    } else {
        None
    }
}

fn shared_start(a: &str, b: &str) -> usize {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).count()
}

/// Gives the last folder of `path` that exists, and the name of the next
/// part of the path, which does not exist or is not a folder.
fn last_existing_folder(path: &Path) -> Option<(PathBuf, String)> {
    let mut folder = PathBuf::new();
    for component in path.components() {
        let next = folder.join(component);
        if next.is_dir() {
            folder = next;
            continue;
        }
        return match component {
            Component::Normal(name) => Some((folder, name.to_string_lossy().into_owned())),
            _ => None,
        };
    }
    None
}

struct Entry {
    path: PathBuf,
    name: String,
    /// The name in lower case, for the match and the sort.
    key: String,
    is_dir: bool,
    modified: SystemTime,
}

impl Entry {
    fn compare_names(&self, other: &Self) -> std::cmp::Ordering {
        self.key
            .cmp(&other.key)
            .then_with(|| self.name.cmp(&other.name))
    }
}

/// Reads the entries of a directory. A hidden entry (name starts with `.`)
/// is read only when `hidden` is true. A symbolic link counts as its target,
/// and a link with no target is not read.
fn read_entries(directory: &Path, hidden: bool, cancel: &CancelToken) -> Vec<Entry> {
    let Ok(read_dir) = fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for entry in read_dir.flatten() {
        if cancel.is_cancelled() {
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && !hidden {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        entries.push(Entry {
            key: name.to_lowercase(),
            name,
            path,
            is_dir: metadata.is_dir(),
            modified: metadata.modified().unwrap_or(UNIX_EPOCH),
        });
    }
    entries
}

/// The text form of a path in the rows and in the query.
struct Display<'a> {
    home: &'a Path,
    tilde: bool,
}

impl Display<'_> {
    fn text(&self, path: &Path) -> String {
        if self.tilde {
            if let Ok(rest) = path.strip_prefix(self.home) {
                if rest.as_os_str().is_empty() {
                    return "~".to_owned();
                }
                return format!("~/{}", rest.to_string_lossy());
            }
        }
        path.to_string_lossy().into_owned()
    }

    fn directory_query(&self, path: &Path) -> String {
        let mut text = self.text(path);
        if !text.ends_with('/') {
            text.push('/');
        }
        text
    }
}

fn entry_row(path: &Path, is_dir: bool, display: &Display) -> Row {
    let title = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| display.text(path));
    let (kind, enter) = if is_dir {
        (
            RowKind::Directory,
            Action::SetQuery(display.directory_query(path)),
        )
    } else {
        (RowKind::File, Action::OpenFile(path.to_path_buf()))
    };
    Row::new(kind, title, enter)
        .with_subtitle(display.text(path))
        .with_icon(Icon::ForFile(path.to_path_buf()))
        .with_local_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/owner";

    fn parse_path(query: &str) -> Option<PathBuf> {
        parse(query, Path::new(HOME)).map(|query_path| query_path.path)
    }

    #[test]
    fn an_absolute_path_is_a_path() {
        assert_eq!(parse_path("/usr/share/"), Some("/usr/share".into()));
    }

    #[test]
    fn a_tilde_is_the_home_directory() {
        assert_eq!(parse_path("~"), Some(HOME.into()));
        assert_eq!(parse_path("~/"), Some(HOME.into()));
        assert_eq!(parse_path("~/a/b"), Some("/home/owner/a/b".into()));
        assert!(parse("~/a", Path::new(HOME)).unwrap().tilde);
    }

    #[test]
    fn a_double_slash_after_the_tilde_stays_below_the_home_directory() {
        assert_eq!(parse_path("~//a"), Some("/home/owner/a".into()));
    }

    #[test]
    fn text_that_is_not_a_path_is_not_a_path() {
        assert_eq!(parse_path("firefox"), None);
        assert_eq!(parse_path("42/2"), None);
        assert_eq!(parse_path("~owner/a"), None);
        assert_eq!(parse_path(""), None);
        assert_eq!(parse_path("file://host/a"), None);
    }

    #[test]
    fn spaces_and_line_ends_around_the_path_have_no_effect() {
        assert_eq!(parse_path(" \t/a/b.png\r\n"), Some("/a/b.png".into()));
    }

    #[test]
    fn quotation_marks_around_the_path_have_no_effect() {
        assert_eq!(parse_path("\"/a/b c.png\""), Some("/a/b c.png".into()));
        assert_eq!(parse_path("'~/b.png'"), Some("/home/owner/b.png".into()));
        assert_eq!(parse_path(" \"/a/b.png\"\n"), Some("/a/b.png".into()));
    }

    #[test]
    fn one_quotation_mark_is_part_of_the_text() {
        assert_eq!(parse_path("\"/a/b.png"), None);
    }

    #[test]
    fn a_file_url_is_a_path_with_its_percent_codes_decoded() {
        assert_eq!(
            parse_path("file:///a/b%20c%C3%A4.png"),
            Some("/a/b cä.png".into())
        );
        assert_eq!(
            parse_path("file://localhost/a/b.png"),
            Some("/a/b.png".into())
        );
        assert_eq!(parse_path("\"file:///a/b.png\""), Some("/a/b.png".into()));
        assert!(!parse("file:///a", Path::new(HOME)).unwrap().tilde);
    }

    #[test]
    fn a_percent_sign_without_two_hex_digits_stays() {
        assert_eq!(parse_path("file:///a/100%"), Some("/a/100%".into()));
        assert_eq!(parse_path("file:///a/%zz"), Some("/a/%zz".into()));
    }

    #[test]
    fn the_contains_rank_puts_the_same_name_first() {
        assert_eq!(contains_rank("l0l3", "l0l3"), Some(0));
        assert_eq!(contains_rank("l0l3-image", "l0l3"), Some(1));
        assert_eq!(contains_rank("old-l0l3", "l0l3"), Some(2));
        assert_eq!(contains_rank("h4", "l0l3"), None);
    }

    #[test]
    fn the_display_uses_a_tilde_only_below_the_home_directory() {
        let display = Display {
            home: Path::new(HOME),
            tilde: true,
        };

        assert_eq!(display.text(Path::new("/home/owner/a")), "~/a");
        assert_eq!(display.text(Path::new("/home/owner")), "~");
        assert_eq!(display.text(Path::new("/usr")), "/usr");
        assert_eq!(display.directory_query(Path::new("/home/owner")), "~/");
        assert_eq!(display.directory_query(Path::new("/")), "/");
    }
}
