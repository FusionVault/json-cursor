//! How long a message takes: json-cursor against serde_json, on 200,000 bid-like messages of about
//! 250 bytes, one message at a time. Run it in release, on a quiet machine:
//!
//! ```bash
//! cargo run --release --example speed
//! ```
use std::hint::black_box;
use std::time::Instant;

use json_cursor::{Cursor, Kind};

const N: usize = 200_000;

fn message(i: usize) -> Vec<u8> {
    format!(
        "{{\"auction\":{},\"bidder\":{},\"price\":{},\"channel\":\"channel-{}\",\"url\":\"https://www.nexmark.com/a/b/item.htm?query={}\",\"dateTime\":{},\"day\":\"2024-01-{:02}\",\"active\":{},\"extra\":\"{}\",\"rate\":{}.25}}",
        1000 + i % 5000, 2000 + i % 9000, 100 + i % 100000, i % 10000, i, 1_730_000_000_000u64 + i as u64, 1 + i % 28, i % 2 == 0, "x".repeat(40), i % 97
    )
    .into_bytes()
}

/// Every key and value of a flat object, read and folded into a number so none is optimised away.
fn walk(msg: &[u8], keys: &mut String, text: &mut String) -> Result<usize, json_cursor::Error> {
    let mut c = Cursor::from_bytes(msg)?;
    let mut sum = 0;
    let mut key = c.first_key(keys)?.map(str::len);
    while let Some(k) = key {
        sum += k + match c.peek()? {
            Kind::String => c.string(text)?.len(),
            Kind::Number => c.number()?.text().len(),
            Kind::Bool => usize::from(c.bool()?),
            Kind::Null => c.null().map(|()| 0)?,
            Kind::Object | Kind::Array => c.skip().map(|()| 0)?,
        };
        key = c.next_key(keys)?.map(str::len);
    }
    c.finish()?;
    Ok(sum)
}

/// The median ns per message over 11 rounds.
fn time(name: &str, msgs: &[Vec<u8>], mut each: impl FnMut(&[u8]) -> usize) {
    let mut rounds: Vec<f64> = (0..11)
        .map(|_| {
            let start = Instant::now();
            let mut sum = 0;
            for m in msgs {
                sum += each(black_box(m));
            }
            black_box(sum);
            start.elapsed().as_nanos() as f64 / msgs.len() as f64
        })
        .collect();
    rounds.sort_by(f64::total_cmp);
    println!("{name:40} {:7.1} ns per message", rounds[5]);
}

fn main() {
    let msgs: Vec<Vec<u8>> = (0..N).map(message).collect();
    let (mut keys, mut text) = (String::new(), String::new());
    time("json-cursor: every key and value", &msgs, |m| {
        walk(m, &mut keys, &mut text).unwrap()
    });
    time("json-cursor: validate", &msgs, |m| {
        usize::from(json_cursor::validate(m).is_ok())
    });
    time("serde_json: IgnoredAny (validate)", &msgs, |m| {
        usize::from(serde_json::from_slice::<serde::de::IgnoredAny>(m).is_ok())
    });
    time("serde_json: Value", &msgs, |m| {
        serde_json::from_slice::<serde_json::Value>(m).map_or(0, |v| v.as_object().map_or(0, |o| o.len()))
    });
}
