//! Timed search on a generated index with the size of the owner's index in
//! R14: 91 755 files in 16 261 directories. Run it with
//! `TMPDIR=$HOME/tmp cargo bench -p petlauncher-index`.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use petlauncher_index::{store, FileEntry, Index, Scope};

const DIRS: usize = 16_261;
const FILES: usize = 91_755;
const RUNS: usize = 200;

const WORDS: &[&str] = &[
    "src",
    "docs",
    "test",
    "notes",
    "palette",
    "zoom",
    "screenshots",
    "final",
    "draft",
    "config",
    "build",
    "assets",
    "images",
    "data",
    "export",
    "report",
    "meeting",
    "cluster",
    "bloom",
    "forge",
    "spur",
    "kukka",
    "daybook",
    "evidence",
    "petlauncher",
    "index",
    "search",
    "main",
    "lib",
    "util",
    "helper",
    "readme",
    "changelog",
    "license",
    "font",
    "size",
    "keys",
    "dark",
    "light",
    "theme",
    "icon",
    "window",
    "settings",
    "query",
    "result",
    "model",
    "server",
    "client",
    "api",
    "schema",
    "deploy",
    "ansible",
    "playbook",
    "inventory",
    "kubeconfig",
    "comfy",
    "output",
    "image",
    "video",
    "audio",
    "backup",
    "archive",
    "old",
    "new",
    "v2",
    "2025",
    "2026",
    "11",
    "24",
    "plan",
    "review",
    "summary",
    "invoice",
    "photo",
    "scan",
];
const EXTENSIONS: &[&str] = &[
    ".rs", ".md", ".png", ".jpg", ".toml", ".json", ".yaml", ".py", ".go", ".txt", ".pdf", ".sh",
    ".html", ".css", ".js", ".csv", ".log", ".svg", "",
];

/// A small deterministic generator, so that each run makes the same tree.
struct Random(u64);

impl Random {
    fn next(&mut self) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 as usize
    }

    fn pick<'a>(&mut self, list: &[&'a str]) -> &'a str {
        list[self.next() % list.len()]
    }

    /// One to `max_words` words with a separator between them.
    fn name(&mut self, max_words: usize) -> String {
        let words = 1 + self.next() % max_words;
        let mut name = String::from(self.pick(WORDS));
        for _ in 1..words {
            name.push(['-', '_', '-', '.'][self.next() % 4]);
            name.push_str(self.pick(WORDS));
        }
        if self.next().is_multiple_of(5) {
            name = name.to_uppercase();
        }
        name
    }
}

fn generate() -> Vec<FileEntry> {
    let mut random = Random(0x5eed_1234_abcd_0001);
    let mut dirs = vec![String::from("/home/owner")];
    let mut entries = Vec::with_capacity(DIRS + FILES);
    while dirs.len() <= DIRS {
        // Most parents are near the top, as in a home directory.
        let near_top = 1 + random.next() % 2000;
        let parent = &dirs[random.next() % dirs.len().min(near_top)];
        let path = format!("{parent}/{}{}", random.name(2), dirs.len());
        entries.push(FileEntry {
            path: PathBuf::from(&path),
            is_dir: true,
            modified: modified(&mut random),
        });
        dirs.push(path);
    }
    for i in 0..FILES {
        let dir = &dirs[1 + random.next() % DIRS];
        let name = match i {
            0 => "palette-zoom.png".to_owned(),
            1..=3 => format!("palette-zoom-{i}.png"),
            _ => format!("{}{}", random.name(3), random.pick(EXTENSIONS)),
        };
        entries.push(FileEntry {
            path: PathBuf::from(format!("{dir}/{name}")),
            is_dir: false,
            modified: modified(&mut random),
        });
    }
    entries
}

fn modified(random: &mut Random) -> SystemTime {
    let seconds = 1_700_000_000 + random.next() % 30_000_000;
    UNIX_EPOCH + Duration::from_secs(seconds as u64)
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Runs `work` `runs` times and gives the median and the maximum.
fn time<T>(runs: usize, mut work: impl FnMut() -> T) -> (f64, f64, T) {
    let mut times = Vec::with_capacity(runs);
    let mut last = None;
    for _ in 0..runs {
        let start = Instant::now();
        last = Some(work());
        times.push(milliseconds(start.elapsed()));
    }
    times.sort_by(f64::total_cmp);
    (times[runs / 2], times[runs - 1], last.unwrap())
}

fn main() {
    let entries = generate();
    let path_bytes: usize = entries.iter().map(|e| e.path.as_os_str().len() + 1).sum();
    let start = Instant::now();
    let index = Index::from_entries(entries, SystemTime::now());
    let build = milliseconds(start.elapsed());
    println!(
        "generated index: {} entries ({DIRS} directories, {FILES} files), {:.1} MB of full paths",
        index.len(),
        path_bytes as f64 / 1e6
    );
    println!(
        "build {build:.1} ms, heap size {:.2} MB",
        index.heap_size() as f64 / 1e6
    );

    println!("search, limit 20, {RUNS} runs: median ms, max ms, hits");
    for text in [
        "palette-zoom",
        "readme",
        "main rs",
        "Zoom",
        "png",
        "e",
        "zzqx",
    ] {
        let (median, max, hits) = time(RUNS, || index.search(text, 20));
        println!("  {text:>14}: {median:6.2} {max:6.2} {:>3}", hits.len());
    }

    let dir = tempfile::tempdir().expect("make a temporary directory");
    let file = dir.path().join(store::FILE_NAME);
    let scope = Scope::new(vec!["~".into()], vec![], PathBuf::from("/home/owner"));
    let (save, _, ()) = time(5, || store::save(&file, &scope, &index).unwrap());
    let size = std::fs::metadata(&file).unwrap().len();
    let (load, load_max, loaded) = time(20, || store::load(&file, &scope).unwrap());
    assert_eq!(loaded.len(), index.len());
    println!(
        "stored file: {:.2} MB, save median {save:.1} ms, load median {load:.1} ms, max {load_max:.1} ms",
        size as f64 / 1e6
    );
}
