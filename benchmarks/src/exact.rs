//! Numbers a double cannot hold: what each parser gives for three of them, and how often each
//! reads a long decimal as the nearest double.

use crate::csv;

/// What each parser gives for three numbers a double cannot hold exactly, as CSV.
pub(crate) fn exact() {
    let numbers = [
        "-237462374673276894279832749832423479823246327846",
        "1e400",
        "0.1000000000000000055511151231257827",
    ];
    println!("parser,read_with,{}", numbers.join(","));
    let row = |parser: &str, how: &str, read: &dyn Fn(&str) -> String| {
        let cells: Vec<String> = numbers.iter().map(|n| csv(&read(n))).collect();
        println!("{parser},{how},{}", cells.join(","));
    };
    row(
        "json-cursor",
        "number()",
        &|n| match json_cursor::Cursor::new(n).number().map(|v| v.text()) {
            Ok(text) if text == n => "the exact text".to_string(),
            Ok(text) => text.to_string(),
            Err(e) => format!("error: {e}"),
        },
    );
    row("jiter", "known_number", &|n| {
        let mut j = jiter::Jiter::new(n.as_bytes());
        let peek = j.peek().expect("a number");
        match j.known_number(peek) {
            Ok(v) => format!("{v:?}"),
            Err(e) => format!("error: {e}"),
        }
    });
    row(
        "serde_json",
        "Value",
        &|n| match serde_json::from_str::<serde_json::Value>(n) {
            Ok(v) => v.to_string(),
            Err(e) => format!("error: {e}"),
        },
    );
    row(
        "sonic-rs",
        "Value",
        &|n| match sonic_rs::from_str::<sonic_rs::Value>(n) {
            Ok(v) => v.to_string(),
            Err(e) => format!("error: {}", e.to_string().lines().next().unwrap_or("")),
        },
    );
    row(
        "simd-json",
        "owned::to_value",
        &|n| match simd_json::owned::to_value(&mut n.as_bytes().to_vec()) {
            Ok(v) => v.to_string(),
            Err(e) => format!("error: {e}"),
        },
    );
}

/// Of a million random decimals with up to 17 digits and exponents from -64 to 63, how many each
/// parser reads as the nearest double, which Rust's own parser gives, as CSV.
pub(crate) fn rounding() {
    const N: usize = 1_000_000;
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let texts: Vec<String> = (0..N)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            format!("{}e{}", x % 100_000_000_000_000_000, (x >> 57) as i32 - 64)
        })
        .collect();
    println!("parser,read_with,nearest_double,another_double");
    let count = |parser: &str, how: &str, read: &dyn Fn(&str) -> Option<f64>| {
        let nearest = texts
            .iter()
            .filter(|t| read(t).map(f64::to_bits) == t.parse::<f64>().ok().map(f64::to_bits))
            .count();
        println!("{parser},{how},{nearest},{}", N - nearest);
    };
    count("json-cursor", "number()?.as_f64()", &|t| {
        json_cursor::Cursor::new(t).number().ok().map(|n| n.as_f64())
    });
    count("jiter", "next_float", &|t| {
        jiter::Jiter::new(t.as_bytes()).next_float().ok()
    });
    count("serde_json", "from_str::<f64>", &|t| serde_json::from_str(t).ok());
    count("sonic-rs", "from_str::<f64>", &|t| sonic_rs::from_str(t).ok());
    count("simd-json", "serde::from_slice::<f64>", &|t| {
        simd_json::serde::from_slice(&mut t.as_bytes().to_vec()).ok()
    });
}
