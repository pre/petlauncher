//! The rows of the file search `f <text>` (R14, R21).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use petlauncher_config::FileSearch;
use petlauncher_core::config::Settings;
use petlauncher_core::{Action, CancelToken, Icon, Provider, Row, RowKind};
use petlauncher_index::{FileEntry, FileIndex, Scope};

/// The largest number of rows of a file search. The path mode has the same
/// limit.
const FILE_ROWS: usize = 50;

/// Files and directories whose name matches the text. The rows have the
/// same keys as the rows of the path mode: Enter opens a file and puts the
/// path of a directory in the query line, Ctrl+Enter shows the item in the
/// file manager, and Shift+Enter copies its path.
pub(crate) struct FileSearchProvider {
    index: Arc<FileIndex>,
    home: PathBuf,
}

impl FileSearchProvider {
    pub(crate) fn new(index: Arc<FileIndex>, home: PathBuf) -> Self {
        Self { index, home }
    }
}

impl Provider for FileSearchProvider {
    fn search(&self, text: &str, _settings: &Settings, _cancel: &CancelToken) -> Vec<Row> {
        self.index
            .search(text, FILE_ROWS)
            .iter()
            .map(|entry| file_row(entry, &self.home))
            .collect()
    }
}

/// The scope of the index for the `[file_search]` settings.
pub(crate) fn scope(file_search: &FileSearch, home: &Path, outside_ignore_files: bool) -> Scope {
    Scope {
        roots: file_search.directories.clone(),
        ignore: file_search.ignore.clone(),
        home: home.to_owned(),
        outside_ignore_files,
    }
}

fn file_row(entry: &FileEntry, home: &Path) -> Row {
    let shown = with_tilde(&entry.path, home);
    let name = entry
        .path
        .file_name()
        .map_or_else(|| shown.clone(), |name| name.to_string_lossy().into_owned());
    let (kind, enter) = if entry.is_dir {
        (RowKind::Directory, Action::SetQuery(format!("{shown}/")))
    } else {
        (RowKind::File, Action::OpenFile(entry.path.clone()))
    };
    Row::new(kind, name, enter)
        .with_subtitle(shown)
        .with_icon(Icon::ForFile(entry.path.clone()))
        .with_local_file(entry.path.clone())
}

/// The path with `~` for the home directory, as the path mode shows it.
fn with_tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    fn entry(path: &str, is_dir: bool) -> FileEntry {
        FileEntry {
            path: path.into(),
            is_dir,
            modified: SystemTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn a_file_row_opens_the_file_and_has_the_file_as_its_local_file() {
        let row = file_row(
            &entry("/home/u/a/palette-zoom.png", false),
            Path::new("/home/u"),
        );

        assert_eq!(row.kind, RowKind::File);
        assert_eq!(row.title, "palette-zoom.png");
        assert_eq!(row.subtitle, "~/a/palette-zoom.png");
        assert_eq!(
            row.enter,
            Action::OpenFile("/home/u/a/palette-zoom.png".into())
        );
        assert_eq!(row.local_file, Some("/home/u/a/palette-zoom.png".into()));
        assert_eq!(row.icon, Icon::ForFile("/home/u/a/palette-zoom.png".into()));
    }

    #[test]
    fn a_directory_row_puts_its_path_in_the_query_line() {
        let row = file_row(
            &entry("/home/u/lolcat/L0L3-IMAGE", true),
            Path::new("/home/u"),
        );

        assert_eq!(row.kind, RowKind::Directory);
        assert_eq!(row.enter, Action::SetQuery("~/lolcat/L0L3-IMAGE/".into()));
        assert_eq!(row.local_file, Some("/home/u/lolcat/L0L3-IMAGE".into()));
    }

    #[test]
    fn a_path_outside_the_home_directory_shows_in_full() {
        let row = file_row(&entry("/mnt/data/L0L3", true), Path::new("/home/u"));

        assert_eq!(row.subtitle, "/mnt/data/L0L3");
        assert_eq!(row.enter, Action::SetQuery("/mnt/data/L0L3/".into()));
    }

    #[test]
    fn the_home_directory_itself_shows_as_tilde() {
        assert_eq!(with_tilde(Path::new("/home/u"), Path::new("/home/u")), "~");
    }

    #[test]
    fn the_scope_has_the_directories_and_the_ignore_entries_of_the_settings() {
        let settings = FileSearch {
            keyword: "f".into(),
            directories: vec!["~".into(), "/mnt/data".into()],
            ignore: vec!["~/tmp".into()],
        };

        let scope = scope(&settings, Path::new("/home/u"), false);

        assert_eq!(scope.roots, ["~", "/mnt/data"]);
        assert_eq!(scope.ignore, ["~/tmp"]);
        assert_eq!(scope.home, PathBuf::from("/home/u"));
        assert!(!scope.outside_ignore_files);
    }
}
