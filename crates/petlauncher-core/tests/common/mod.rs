//! Fixtures for the acceptance tests. Each fixture makes a home directory in
//! a new temporary directory. `tempfile` makes it in `TMPDIR`, so run the
//! tests with `TMPDIR=$HOME/tmp`.

#![allow(dead_code)]

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use petlauncher_core::config::{Settings, Shortcut};
use petlauncher_core::{Action, CancelToken, Engine, Results, Row, RowKind};
use tempfile::TempDir;

pub struct TempHome {
    _root: TempDir,
    pub home: PathBuf,
}

impl TempHome {
    pub fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("petlauncher-core-")
            .tempdir()
            .expect("make a temporary directory");
        let home = root.path().join("home");
        fs::create_dir(&home).expect("make the home directory");
        Self { _root: root, home }
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.home.join(relative)
    }

    pub fn dir(&self, relative: &str) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(&path).expect("make a directory");
        path
    }

    /// Makes a file with the modification time `age_rank` seconds after a
    /// fixed time. A larger rank is newer.
    pub fn file(&self, relative: &str, age_rank: u64) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("make the parent directory");
        File::create(&path).expect("make a file");
        set_modified(&path, age_rank);
        path
    }

    pub fn engine(&self) -> Engine {
        Engine::new(&self.home)
    }
}

pub fn set_modified(path: &Path, age_rank: u64) {
    let time = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + age_rank);
    File::open(path)
        .and_then(|file| file.set_modified(time))
        .expect("set the modification time");
}

pub fn search(engine: &Engine, query: &str) -> Results {
    search_with(engine, query, &Settings::default(), &[])
}

pub fn search_with(
    engine: &Engine,
    query: &str,
    settings: &Settings,
    shortcuts: &[Shortcut],
) -> Results {
    engine.search(query, settings, shortcuts, &CancelToken::new())
}

pub fn titles(results: &Results) -> Vec<&str> {
    results.rows.iter().map(|row| row.title.as_str()).collect()
}

pub fn row_index(results: &Results, title: &str) -> usize {
    results
        .rows
        .iter()
        .position(|row| row.title == title)
        .unwrap_or_else(|| panic!("no row {title:?} in {:?}", titles(results)))
}

/// A provider that gives one row with the title `"<name>: <text>"`.
pub fn echo_row(kind: RowKind, name: &str, text: &str) -> Row {
    Row::new(kind, format!("{name}: {text}"), Action::Nothing)
}

/// A calculator that knows the sums of the acceptance steps.
pub fn fake_calculator(text: &str, _: &Settings, _: &CancelToken) -> Vec<Row> {
    let result = match text {
        "42.3+68.7+14.1+0.5" => "125.6",
        "42.3+68.7" => "111",
        "42+1" => "43",
        _ => return Vec::new(),
    };
    vec![Row::new(
        RowKind::Calculation,
        result,
        Action::Copy(result.to_owned()),
    )]
}

/// Applications with a `.desktop` file in `directory`.
pub fn fake_applications(
    directory: PathBuf,
) -> impl Fn(&str, &Settings, &CancelToken) -> Vec<Row> + Send + Sync {
    move |text, _, _| {
        ["Firefox", "Files"]
            .into_iter()
            .filter(|name| name.to_lowercase().starts_with(&text.to_lowercase()))
            .map(|name| {
                let desktop_file = directory.join(format!("{}.desktop", name.to_lowercase()));
                Row::new(
                    RowKind::Application,
                    name,
                    Action::LaunchDesktopEntry(desktop_file.clone()),
                )
                .with_local_file(desktop_file)
            })
            .collect()
    }
}
