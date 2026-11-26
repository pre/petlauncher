//! Downloads the ECB reference rates once and stores them in a directory.
//!
//! Usage: `cargo run -p petlauncher-calc --example fetch_rates -- <directory>`

use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let Some(dir) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("Usage: fetch_rates <directory>");
        std::process::exit(2);
    };
    match petlauncher_calc::update_cache(&dir, Duration::from_secs(10)) {
        Ok(rates) => {
            let codes: Vec<&str> = rates.currencies().collect();
            println!("Rates of {}: {}", rates.date(), codes.join(" "));
        }
        Err(error) => {
            eprintln!("Cannot update the rates: {error}");
            std::process::exit(1);
        }
    }
}
