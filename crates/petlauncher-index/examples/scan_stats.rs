//! A manual measurement: scans a home directory with the initial ignore list
//! of R14, and prints counts, times and sizes. It prints no file names, and
//! it writes the stored index only to a temporary directory in `TMPDIR`.
//!
//! `TMPDIR=$HOME/tmp cargo run --release -p petlauncher-index --example scan_stats -- [HOME [PATH...]]`
//!
//! For each `PATH`, relative to `HOME`, it tells if a search by the file
//! name of `PATH` without the extension has that file in the first 20 hits.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use petlauncher_index::{scan, store, Index, Scope};

const INITIAL_IGNORE: [&str; 7] = [
    "node_modules",
    "target",
    "__pycache__",
    "~/go/pkg/mod",
    "~/tmp",
    "git-worktrees",
    "*-wt",
];

fn seconds(duration: Duration) -> String {
    format!("{:.3} s", duration.as_secs_f64())
}

fn timed_scan(scope: &Scope, threads: usize) -> (Duration, Index) {
    let start = Instant::now();
    let index = scan(scope, threads, &AtomicBool::new(false)).expect("scan");
    (start.elapsed(), index)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let home = args
        .next()
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .expect("give the home directory");
    let checks: Vec<PathBuf> = args.map(|path| home.join(path)).collect();
    let scope = Scope::new(
        vec!["~".into()],
        INITIAL_IGNORE.map(str::to_owned).to_vec(),
        home,
    );

    let mut index = Index::default();
    for (threads, runs) in [(0, 5), (32, 5), (1, 3)] {
        let times: Vec<_> = (0..runs)
            .map(|_| {
                let (time, scanned) = timed_scan(&scope, threads);
                index = scanned;
                seconds(time)
            })
            .collect();
        println!(
            "scan, threads {threads} (0 = automatic): {}",
            times.join(", ")
        );
    }

    let (mut files, mut dirs, mut links, mut other) = (0, 0, 0, 0);
    let (mut path_bytes, mut name_bytes) = (0, 0);
    for entry in index.iter() {
        let kind = fs::symlink_metadata(&entry.path).map(|m| m.file_type());
        match kind {
            Ok(kind) if kind.is_dir() => dirs += 1,
            Ok(kind) if kind.is_symlink() => links += 1,
            Ok(kind) if kind.is_file() => {
                files += 1;
                path_bytes += entry.path.as_os_str().len() + 1;
                name_bytes += entry.path.file_name().map_or(0, |n| n.len()) + 1;
            }
            _ => other += 1,
        }
    }
    println!(
        "entries {}: regular files {files}, directories {dirs}, symbolic links {links}, other or gone {other}, scan errors {}",
        index.len(),
        index.errors()
    );
    println!(
        "regular files as plain text: full paths {:.2} MB, names {:.2} MB",
        path_bytes as f64 / 1e6,
        name_bytes as f64 / 1e6
    );
    println!("index heap size {:.2} MB", index.heap_size() as f64 / 1e6);

    let dir = tempfile::tempdir().expect("make a temporary directory");
    let file = dir.path().join(store::FILE_NAME);
    let start = Instant::now();
    store::save(&file, &scope, &index).expect("save");
    let save = start.elapsed();
    let size = fs::metadata(&file).expect("stored file").len();
    let mut loads: Vec<_> = (0..10)
        .map(|_| {
            let start = Instant::now();
            let loaded = store::load(&file, &scope).expect("load");
            assert_eq!(loaded.len(), index.len());
            start.elapsed()
        })
        .collect();
    loads.sort();
    println!(
        "stored file {:.2} MB, save {}, load median {}, load max {}",
        size as f64 / 1e6,
        seconds(save),
        seconds(loads[5]),
        seconds(loads[9])
    );

    for text in ["palette-zoom", "readme", "e"] {
        let mut times: Vec<_> = (0..100)
            .map(|_| {
                let start = Instant::now();
                let hits = index.search(text, 20);
                (start.elapsed(), hits.len())
            })
            .collect();
        times.sort();
        println!(
            "search {text:?}, limit 20: median {:.2} ms, max {:.2} ms, {} hits",
            times[50].0.as_secs_f64() * 1000.0,
            times[99].0.as_secs_f64() * 1000.0,
            times[0].1
        );
    }

    for path in checks {
        let name = path.file_stem().expect("a file name").to_string_lossy();
        let found = index.search(&name, 20).iter().any(|hit| hit.path == path);
        println!(
            "{} in the first 20 hits of {name:?}: {found}",
            path.display()
        );
    }
}
