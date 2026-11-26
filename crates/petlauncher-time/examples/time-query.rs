//! Loads the system files, answers each query of the command line and shows
//! the load time and the query time.
//!
//! `cargo run --release -p petlauncher-time --example time-query -- "17:00 in las vegas"`

use std::hint::black_box;
use std::time::Instant;

use petlauncher_time::jiff::ZonedDateTime;
use petlauncher_time::Cities;

const REPEATS: u32 = 10_000;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let cities = Cities::load_system()?;
    println!("{} cities loaded in {:.1?}", cities.len(), start.elapsed());

    let now = ZonedDateTime::now();
    for query in std::env::args().skip(1) {
        let start = Instant::now();
        let answers = cities.answer(&query, &now);
        let first = start.elapsed();
        let start = Instant::now();
        for _ in 0..REPEATS {
            black_box(cities.answer(black_box(&query), &now));
        }
        let mean = start.elapsed() / REPEATS;
        println!("\n{query:?}: first call {first:.1?}, mean of {REPEATS} calls {mean:.1?}");
        for answer in answers {
            println!(
                "  {}  |  {}  |  copy: {:?}",
                answer.title, answer.subtitle, answer.copy
            );
        }
    }
    Ok(())
}
