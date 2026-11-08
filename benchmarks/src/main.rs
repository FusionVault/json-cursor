//! json-cursor against jiter, serde_json, sonic-rs and simd-json, on the same messages.
//!
//! ```bash
//! cargo build --release
//! taskset -c 15 target/release/json-cursor-benchmarks speed > results/speed.csv   # on one idle core
//! target/release/json-cursor-benchmarks strict > results/strict.csv
//! target/release/json-cursor-benchmarks exact > results/exact.csv
//! target/release/json-cursor-benchmarks rounding > results/rounding.csv
//! python3 charts.py
//! ```
//!
//! `speed` times a thread's CPU, not the wall clock, so a busy machine slows it less: the median of
//! 15 rounds per parser, the rounds taken in turn. Every parser reads every value into the same
//! sum, which must agree, and each uses its own natural way to do it: a pull walk for json-cursor
//! and jiter, a serde visitor that builds nothing for serde_json, sonic-rs and simd-json, and
//! serde_json's `Value` for scale. simd-json needs a mutable copy of each message, as a Kafka
//! payload would, and the copy is counted.

mod exact;
mod messages;
mod parsers;
mod profile;
mod rng;
mod speed;
mod strict;
mod walk;

use parsers::Job;

/// A CSV cell: quoted when it holds a comma, a quote or a line break.
fn csv(cell: &str) -> String {
    if cell.contains([',', '"', '\n']) {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("speed") => speed::speed(&args[2..]),
        Some("strict") => strict::strict(),
        Some("deep") => strict::deep(&args[2]),
        Some("exact") => exact::exact(),
        Some("rounding") => exact::rounding(),
        Some("profile") => {
            let job = if args.get(4).is_some_and(|j| j == "validate") {
                Job::Validate
            } else {
                Job::Read
            };
            profile::profile(&args[2], &args[3], job)
        }
        _ => eprintln!(
            "json-cursor-benchmarks speed [shape...] | strict | exact | rounding | profile <shape> <parser> [validate]"
        ),
    }
}
