//! The number of starts of each entry, kept in a small text file.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The number of starts for each desktop file ID.
pub type UsageCounts = HashMap<String, u32>;

/// The start counts in the file `app-usage.tsv` of a directory that the
/// caller gives. Each line is the count, a tab and the desktop file ID.
#[derive(Debug)]
pub struct UsageStore {
    file: PathBuf,
    counts: UsageCounts,
}

impl UsageStore {
    pub const FILE_NAME: &'static str = "app-usage.tsv";

    /// Reads the file in `dir`. A missing file or directory gives no counts.
    /// A line that is not a count, a tab and an ID is ignored.
    pub fn open(dir: &Path) -> io::Result<UsageStore> {
        let file = dir.join(Self::FILE_NAME);
        let text = match fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error),
        };
        let counts = text
            .lines()
            .filter_map(|line| {
                let (count, id) = line.split_once('\t')?;
                Some((id.to_string(), count.parse().ok()?)).filter(|(id, _)| !id.is_empty())
            })
            .collect();
        Ok(UsageStore { file, counts })
    }

    pub fn counts(&self) -> &UsageCounts {
        &self.counts
    }

    /// Adds one start of `id` and writes the file. Makes the directory
    /// when it is missing.
    pub fn record_start(&mut self, id: &str) -> io::Result<()> {
        let count = self.counts.entry(id.to_string()).or_default();
        *count = count.saturating_add(1);

        let mut lines: Vec<(&String, &u32)> = self.counts.iter().collect();
        lines.sort();
        let text: String = lines
            .iter()
            .map(|(id, count)| format!("{count}\t{id}\n"))
            .collect();

        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir)?;
        }
        // A rename replaces the file in one step, so a crash does not leave
        // half a file.
        let partial = self.file.with_extension("tsv.partial");
        fs::write(&partial, text)?;
        fs::rename(&partial, &self.file)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn gives_no_counts_when_the_directory_is_missing() {
        let dir = tempfile::tempdir().unwrap();

        let store = UsageStore::open(&dir.path().join("missing")).unwrap();

        assert!(store.counts().is_empty());
    }

    #[test]
    fn counts_each_start_and_keeps_the_counts_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("petlauncher");
        let mut store = UsageStore::open(&cache).unwrap();

        store.record_start("org.gnome.Terminal.desktop").unwrap();
        store.record_start("org.gnome.Terminal.desktop").unwrap();
        store.record_start("google-chrome.desktop").unwrap();
        let reopened = UsageStore::open(&cache).unwrap();

        let expected = UsageCounts::from([
            ("org.gnome.Terminal.desktop".to_string(), 2),
            ("google-chrome.desktop".to_string(), 1),
        ]);
        assert_eq!(store.counts(), &expected);
        assert_eq!(reopened.counts(), &expected);
    }

    #[test]
    fn ignores_lines_that_are_not_a_count_and_an_id() {
        let dir = tempfile::tempdir().unwrap();
        let text = "3\tfiles.desktop\nnot a line\nx\tbad.desktop\n\t\n7\tterminal.desktop\n";
        fs::write(dir.path().join(UsageStore::FILE_NAME), text).unwrap();

        let store = UsageStore::open(dir.path()).unwrap();

        let expected = UsageCounts::from([
            ("files.desktop".to_string(), 3),
            ("terminal.desktop".to_string(), 7),
        ]);
        assert_eq!(store.counts(), &expected);
    }

    #[test]
    fn writes_one_line_for_each_id_in_id_order() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = UsageStore::open(dir.path()).unwrap();

        store.record_start("b.desktop").unwrap();
        store.record_start("a.desktop").unwrap();
        store.record_start("b.desktop").unwrap();

        let text = fs::read_to_string(dir.path().join(UsageStore::FILE_NAME)).unwrap();
        assert_eq!(text, "1\ta.desktop\n2\tb.desktop\n");
    }

    #[test]
    fn reports_an_error_when_the_directory_cannot_be_made() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("cache");
        let mut store = UsageStore::open(&cache).unwrap();
        fs::write(&cache, "a file in the place of the directory").unwrap();

        assert!(store.record_start("a.desktop").is_err());
    }
}
