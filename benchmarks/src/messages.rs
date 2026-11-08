//! The three kinds of message every parser reads.

use std::fmt::Write as _;

/// A Nexmark-like bid, about 250 bytes: integers, a timestamp, short text.
fn bid(i: usize) -> String {
    format!(
        "{{\"auction\":{},\"bidder\":{},\"price\":{},\"channel\":\"channel-{}\",\"url\":\"https://www.nexmark.com/a/b/item.htm?query={}\",\"dateTime\":{},\"day\":\"2024-01-{:02}\",\"active\":{},\"extra\":\"{}\",\"rate\":{}.25}}",
        1000 + i % 5000, 2000 + i % 9000, 100 + i % 100000, i % 10000, i, 1_730_000_000_000u64 + i as u64, 1 + i % 28, i.is_multiple_of(2), "x".repeat(40), i % 97
    )
}

fn hex(seed: usize, n: usize) -> String {
    let mut x = seed as u64 ^ 0x9E37_79B9_7F4A_7C15;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            char::from(b"0123456789abcdef"[(x % 16) as usize])
        })
        .collect()
}

/// A Nostr-like event, about 900 bytes: long hex strings, tags, and text with escapes and unicode.
fn event(i: usize) -> String {
    let content = format!(
        "Day {i} on the relay. Prices moved \\\"a lot\\\" today:\\n- café au lait up 3%\\n- 漢字 lessons booked\\n- 😀 for everyone who reads this far, and a long tail of plain words so the text runs on like a real note does {}",
        "and on ".repeat(i % 7)
    );
    format!(
        "{{\"id\":\"{}\",\"pubkey\":\"{}\",\"created_at\":{},\"kind\":1,\"tags\":[[\"e\",\"{}\"],[\"p\",\"{}\"]],\"content\":\"{content}\",\"sig\":\"{}\"}}",
        hex(i, 64), hex(i + 1, 64), 1_730_000_000 + i, hex(i + 2, 64), hex(i + 3, 64), hex(i + 4, 128)
    )
}

/// A GeoJSON feature, about 4 KB: a polygon of 120 points, numbers almost all the way.
fn feature(i: usize) -> String {
    let mut ring = String::new();
    for p in 0..120 {
        let a = p as f64 / 120.0 * std::f64::consts::TAU;
        if p > 0 {
            ring.push(',');
        }
        write!(
            ring,
            "[{:.7},{:.7}]",
            151.2 + 0.01 * a.cos() + i as f64 * 1e-4,
            -33.8 + 0.01 * a.sin()
        )
        .unwrap();
    }
    format!(
        "{{\"type\":\"Feature\",\"properties\":{{\"name\":\"zone-{i}\",\"height\":{}.5,\"open\":true}},\"geometry\":{{\"type\":\"Polygon\",\"coordinates\":[[{ring}]]}}}}",
        i % 400
    )
}

pub(crate) type Shape = (&'static str, fn(usize) -> String, usize);

pub(crate) const SHAPES: [Shape; 3] = [
    ("bids", bid, 200_000),
    ("events", event, 50_000),
    ("features", feature, 10_000),
];
