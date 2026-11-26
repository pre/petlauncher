//! Manual check against the real directories of this desktop. It only reads.
//!
//! `cargo run -p petlauncher-apps --example list -- [QUERY...]`
//!
//! Prints the number of applications and settings pages, the settings pages,
//! the browsers, the files that could not be read, and the best results for
//! each query. Usage counts are not used.

use std::time::Instant;

use petlauncher_apps::{gnome, Applications, EntryKind, UsageCounts};

fn main() {
    let start = Instant::now();
    let apps = Applications::load(
        petlauncher_apps::Environment::from_process(),
        gnome::is_settings_page,
    );
    let load_time = start.elapsed();
    let start = Instant::now();
    let changed = apps.has_changed();
    let check_time = start.elapsed();

    let pages: Vec<_> = apps
        .entries()
        .iter()
        .filter(|entry| entry.kind == EntryKind::SettingsPage)
        .collect();
    println!(
        "applications: {}, settings pages: {}, load: {load_time:?}, change check: {check_time:?} ({changed})",
        apps.entries().len() - pages.len(),
        pages.len()
    );
    for page in &pages {
        println!("page\t{}\t{}\t{}", page.id, page.name, page.exec.join(" "));
    }
    for browser in apps.browsers() {
        println!(
            "browser\t{}\t{}\t{}",
            browser.id,
            browser.name,
            browser.path.display()
        );
    }
    for error in apps.errors() {
        println!("error\t{}\t{}", error.path.display(), error.message);
    }
    for entry in apps.entries() {
        println!("entry\t{}\t{:?}\t{}", entry.id, entry.kind, entry.name);
    }
    for query in std::env::args().skip(1) {
        let start = Instant::now();
        let hits = apps.search(&query, 5, &UsageCounts::new());
        let search_time = start.elapsed();
        let names: Vec<String> = hits
            .iter()
            .map(|hit| format!("{} ({})", hit.entry.name, hit.score))
            .collect();
        println!("search\t{query}\t{search_time:?}\t{}", names.join(", "));
    }
}
