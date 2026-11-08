//! A number's values against Rust's own parsers and a long-hand reference: `as_f64` bit for bit
//! against `str::parse::<f64>`, `as_i64` against exact arithmetic on every digit, and against
//! `str::parse::<i64>` for numbers written with digits only.

use json_cursor::Cursor;

/// The exact whole value of a number's text, the long way: every digit kept, in a `u128`.
fn whole(text: &str) -> Option<i64> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(e) => {
            let exp = &body[e + 1..];
            // an exponent past i128: only its sign matters
            let far = if exp.starts_with('-') {
                -(1i128 << 100)
            } else {
                1i128 << 100
            };
            (&body[..e], exp.parse::<i128>().unwrap_or(far))
        }
        None => (body, 0),
    };
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let all = format!("{int}{frac}");
    let trimmed = all.trim_start_matches('0');
    if trimmed.is_empty() {
        return Some(0);
    }
    let significant = trimmed.trim_end_matches('0');
    let scale = exponent - frac.len() as i128 + (trimmed.len() - significant.len()) as i128;
    if scale < 0 || significant.len() as i128 + scale > 20 {
        return None;
    }
    let value: u128 = significant.parse::<u128>().ok()? * 10u128.pow(scale as u32);
    if negative {
        (value <= 1 << 63).then(|| (value as i128).wrapping_neg() as i64)
    } else {
        i64::try_from(value).ok()
    }
}

#[track_caller]
fn check(text: &str) {
    let n = Cursor::new(text).number().unwrap_or_else(|e| panic!("{text}: {e}"));
    assert_eq!(n.text(), text);
    let want = text.parse::<f64>().unwrap();
    assert_eq!(
        n.as_f64().to_bits(),
        want.to_bits(),
        "{text}: {} not {want}",
        n.as_f64()
    );
    assert_eq!(n.as_i64(), whole(text), "{text}");
    assert_eq!(n.is_integer(), !text.contains(['.', 'e', 'E']), "{text}");
    if n.is_integer() {
        assert_eq!(
            n.as_i64(),
            text.parse::<i64>().ok(),
            "{text}: as Rust's parser reads it"
        );
    }
}

#[test]
fn the_edges_of_the_fast_path_and_of_i64() {
    for text in [
        "0",
        "-0",
        "0.0",
        "-0.0",
        "1",
        "-1",
        "0.1",
        "0.3",
        "2.5",
        "1e22",
        "1e23",
        "-1e22",
        "1e-22",
        "1e-23",
        "9007199254740992",
        "9007199254740993",
        "-9007199254740993",
        "9007199254740992e22",
        "123456789012345678",
        "1234567890123456789",
        "12345678901234567890",
        "0.1234567890123456789",
        "1.7976931348623157e308",
        "1.7976931348623159e308",
        "1e400",
        "-1e400",
        "4.9e-324",
        "2.4e-324",
        "2.2250738585072014e-308",
        "1e-400",
        "9223372036854775807",
        "-9223372036854775808",
        "9223372036854775808",
        "-9223372036854775809",
        "100000000000000000000",
        "0.000001",
        "1E+2",
        "1e+0",
        "1e-0",
        "123.456e-2",
        "7.0",
        "1e3",
        "0.1000000000000000055511151231257827",
        "-237462374673276894279832749832423479823246327846",
        "1e0000000000000000000022",
        "1e-99999999999999999999",
        "1e99999999999999999999",
        // whole numbers however written, and the edges of the disguised fast path
        "700e-2",
        "7.000",
        "-7.0e0",
        "10e-1",
        "1.5",
        "0.0e5",
        "-0e-9",
        "1000000000000000000000e-3",
        "92233720368547758070e-1",
        "-92233720368547758080e-1",
        "92233720368547758080e-1",
        "123e30",
        "9007199254740992e15",
        "9007199254740993e15",
        "1e37",
        "1e38",
        "4503599627370496e16",
        "0.00000000000000000000000000000000001e35",
        "12345678901234567890000e-4",
        "1.00000000000000000000",
    ] {
        check(text);
    }
}

/// xorshift64: the same numbers on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[test]
fn random_numbers_read_as_rusts_parsers_read_them() {
    let cases: usize = std::env::var("JSON_CURSOR_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300_000);
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    for _ in 0..cases {
        let mut t = String::new();
        if rng.below(3) == 0 {
            t.push('-');
        }
        // integer part: 1 to 25 digits, so both sides of 19 digits and of 2^53 are crossed
        let int_digits = 1 + rng.below(25) as usize;
        if rng.below(6) == 0 {
            t.push('0');
        } else {
            t.push(char::from(b'1' + rng.below(9) as u8));
            for _ in 1..int_digits {
                t.push(char::from(b'0' + rng.below(10) as u8));
            }
        }
        if rng.below(2) == 0 {
            t.push('.');
            for _ in 0..1 + rng.below(20) {
                t.push(char::from(b'0' + rng.below(10) as u8));
            }
        }
        if rng.below(3) == 0 {
            t.push(if rng.below(2) == 0 { 'e' } else { 'E' });
            match rng.below(3) {
                0 => t.push('-'),
                1 => t.push('+'),
                _ => {}
            }
            // mostly near the fast path's ±22, sometimes far past a double's range
            let e = if rng.below(4) == 0 {
                rng.below(400)
            } else {
                rng.below(30)
            };
            t.push_str(&e.to_string());
        }
        check(&t);
    }
}
