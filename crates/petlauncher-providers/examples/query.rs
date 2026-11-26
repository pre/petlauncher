//! Starts a `Launcher` on the real directories of the user and prints the
//! rows for each query of the command line.
//!
//! `cargo run -p petlauncher-providers --example query -- fire "f palette-zoom"`
//!
//! The program reads the settings and does not watch or change them. It
//! does not write the autostart file and does not start an application. It
//! can write the cache files (the file index and the currency rates).

use std::time::Instant;

use petlauncher_core::{Action, CancelToken, Results};
use petlauncher_providers::{Dirs, Hooks, Launcher};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dirs = Dirs::from_env().ok_or("HOME is not an absolute path")?;
    let mut hooks = Hooks::new(petlauncher_apps::gnome::is_settings_page, |_| {});
    hooks.watch_config = false;

    let start = Instant::now();
    let launcher = Launcher::start(dirs, hooks)?;
    let started = start.elapsed();
    launcher.wait_idle();
    println!(
        "start {started:.1?}, background work {:.1?}",
        start.elapsed()
    );
    for problem in launcher.problems() {
        println!("problem: {problem}");
    }

    for query in std::env::args().skip(1) {
        let start = Instant::now();
        let results = launcher.search(&query, &CancelToken::new());
        println!("\n{query:?}: {:.1?}", start.elapsed());
        print_results(&results);
    }
    Ok(())
}

fn print_results(results: &Results) {
    println!(
        "  mode {:?}, {} rows, selected {:?}",
        results.mode,
        results.rows.len(),
        results.selected
    );
    for row in results.rows.iter().take(12) {
        println!("  {:?}: {}", row.kind, row.title);
        if !row.subtitle.is_empty() {
            println!("      {}", row.subtitle);
        }
        println!("      Enter: {}", describe(&row.enter));
        if let Some(file) = &row.local_file {
            println!("      file: {}", file.display());
        }
    }
    if results.rows.len() > 12 {
        println!("  ... {} more rows", results.rows.len() - 12);
    }
}

fn describe(action: &Action) -> String {
    match action {
        Action::Copy(text) => format!("copy {text:?}"),
        Action::LaunchDesktopEntry(path) => format!("start {}", path.display()),
        other => format!("{other:?}"),
    }
}
