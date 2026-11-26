//! Shows the dictionary result for words, with the time and the memory.
//!
//! cargo run --release -p petlauncher-dict --example wordnet -- /usr/share/wordnet launcher launch

use std::time::{Duration, Instant};

use petlauncher_dict::{Dictionary, Found};

const RUNS: u32 = 100;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next() else {
        eprintln!("usage: wordnet <wordnet directory> <word>...");
        std::process::exit(2);
    };

    let memory_before = resident_kib();
    let start = Instant::now();
    let dictionary = match Dictionary::open(&path) {
        Ok(dictionary) => dictionary,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let open_time = start.elapsed();
    let memory_after = resident_kib();
    println!(
        "open: {:.1} ms, resident memory: {} KiB before, {} KiB after",
        ms(open_time),
        memory_before,
        memory_after
    );

    for word in args {
        let (entries, first_lookup, lookup_time) = mean_time(|| dictionary.lookup(&word));
        let (suggestions, _, suggestion_time) = mean_time(|| dictionary.suggestions(&word, 10));
        println!(
            "\n{word}: lookup {:.3} ms (first {:.3} ms), suggestions {:.3} ms, mean of {RUNS}",
            ms(lookup_time),
            ms(first_lookup),
            ms(suggestion_time)
        );
        println!("  suggestions: {}", suggestions.join(", "));
        let entries = match entries {
            Ok(entries) => entries,
            Err(error) => {
                println!("  {error}");
                continue;
            }
        };
        for entry in entries {
            let found = match entry.found {
                Found::Exact => "the typed word",
                Found::BaseForm => "base form",
            };
            println!("  {} ({found})", entry.word);
            for (number, meaning) in entry.meanings.iter().enumerate() {
                println!(
                    "    {}. {}: {}",
                    number + 1,
                    meaning.word_class,
                    meaning.definition
                );
                let lists = [
                    ("examples", &meaning.examples),
                    ("synonyms", &meaning.synonyms),
                    ("antonyms", &meaning.antonyms),
                    ("see also", &meaning.see_also),
                    ("more general", &meaning.hypernyms),
                    ("more specific", &meaning.hyponyms),
                ];
                for (name, list) in lists.iter().filter(|(_, list)| !list.is_empty()) {
                    println!("       {name}: {}", list.join(" | "));
                }
            }
        }
    }
}

/// The result, the time of the first call, and the mean time of the next calls.
fn mean_time<T>(f: impl Fn() -> T) -> (T, Duration, Duration) {
    let start = Instant::now();
    let result = f();
    let first = start.elapsed();
    let start = Instant::now();
    for _ in 0..RUNS {
        std::hint::black_box(f());
    }
    (result, first, start.elapsed() / RUNS)
}

fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// The resident memory of this process (VmRSS) in KiB, from `/proc/self/status`.
fn resident_kib() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|value| value.split_whitespace().next()?.parse().ok())
        .unwrap_or(0)
}
