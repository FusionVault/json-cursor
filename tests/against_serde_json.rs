//! The cursor against serde_json, on random documents and on random damage to them. Both must
//! accept the same inputs and read the same values from them. `JSON_CURSOR_CASES` sets how many
//! cases run, 100,000 by default.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use json_cursor::{validate, Cursor, Kind};
use serde_json::{Number, Value};

mod common;
use common::Rng;

/// A value as the cursor reads it: a number keeps its text.
#[derive(Debug)]
enum Tree {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Tree>),
    Object(BTreeMap<String, Tree>),
}

fn read(c: &mut Cursor, buf: &mut String) -> Result<Tree, json_cursor::Error> {
    Ok(match c.peek()? {
        Kind::Null => {
            c.null()?;
            Tree::Null
        }
        Kind::Bool => Tree::Bool(c.bool()?),
        Kind::Number => Tree::Number(c.number()?.text().to_string()),
        Kind::String => Tree::String(c.string(buf)?.to_string()),
        Kind::Array => {
            let mut items = Vec::new();
            let mut more = c.first_item()?;
            while more {
                items.push(read(c, buf)?);
                more = c.next_item()?;
            }
            Tree::Array(items)
        }
        Kind::Object => {
            let mut map = BTreeMap::new();
            let mut key = c.first_key(buf)?.map(str::to_string);
            while let Some(k) = key {
                let v = read(c, buf)?;
                // a repeated key: the last one wins, as in serde_json's map
                map.insert(k, v);
                key = c.next_key(buf)?.map(str::to_string);
            }
            Tree::Object(map)
        }
    })
}

/// The same value. A number's text goes through serde_json's own parser, so both sides parse it by
/// one algorithm.
fn same(ours: &Tree, theirs: &Value) -> bool {
    match (ours, theirs) {
        (Tree::Null, Value::Null) => true,
        (Tree::Bool(a), Value::Bool(b)) => a == b,
        (Tree::Number(text), Value::Number(n)) => serde_json::from_str::<Number>(text).ok().as_ref() == Some(n),
        (Tree::String(a), Value::String(b)) => a == b,
        (Tree::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y)),
        (Tree::Object(a), Value::Object(b)) => {
            a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| same(v, w)))
        }
        _ => false,
    }
}

fn whitespace(rng: &mut Rng, out: &mut String) {
    if rng.below(4) == 0 {
        for _ in 0..1 + rng.below(3) {
            out.push(*rng.pick(&[' ', '\t', '\n', '\r']));
        }
    }
}

fn digits(rng: &mut Rng, n: usize, out: &mut String) {
    for _ in 0..n {
        out.push(char::from(b'0' + rng.below(10) as u8));
    }
}

/// A number in JSON's grammar, within a double's range: up to 31 digits, an exponent up to 99.
fn number(rng: &mut Rng, out: &mut String) {
    if rng.below(4) == 0 {
        out.push('-');
    }
    if rng.below(5) == 0 {
        out.push('0');
    } else {
        out.push(char::from(b'1' + rng.below(9) as u8));
        let n = if rng.below(10) == 0 {
            rng.below(30)
        } else {
            rng.below(6)
        };
        digits(rng, n, out);
    }
    if rng.below(3) == 0 {
        out.push('.');
        let n = 1 + rng.below(6);
        digits(rng, n, out);
    }
    if rng.below(4) == 0 {
        out.push(*rng.pick(&['e', 'E']));
        if rng.below(2) == 0 {
            out.push(*rng.pick(&['+', '-']));
        }
        let n = 1 + rng.below(2);
        digits(rng, n, out);
    }
}

fn string(rng: &mut Rng, out: &mut String) {
    out.push('"');
    for _ in 0..rng.below(8) {
        match rng.below(12) {
            0 => {
                let escapes: [&str; 8] = ["\\\"", "\\\\", "\\/", "\\b", "\\f", "\\n", "\\r", "\\t"];
                let escape = *rng.pick(&escapes);
                out.push_str(escape)
            }
            1 => {
                // a code point of the basic plane, not a surrogate
                let mut code = rng.below(0xFFFF) as u32;
                if (0xD800..=0xDFFF).contains(&code) {
                    code -= 0x800;
                }
                write!(out, "\\u{code:04x}").unwrap();
            }
            2 => {
                let high = 0xD800 + rng.below(0x400);
                let low = 0xDC00 + rng.below(0x400);
                write!(out, "\\u{high:04X}\\u{low:04x}").unwrap();
            }
            3 => out.push(*rng.pick(&['é', '漢', '😀', 'ß', '\u{7f}', '\u{80}'])),
            _ => out.push(char::from(*rng.pick(b"abcxyz019 _-"))),
        }
    }
    out.push('"');
}

/// A whole document: mostly an object, as a message is, sometimes an array or a lone value.
fn document(rng: &mut Rng, out: &mut String) {
    let kind = match rng.below(20) {
        0..=13 => 6,
        14..=17 => 5,
        _ => rng.below(5),
    };
    value_of(rng, kind, 0, out);
}

fn value(rng: &mut Rng, depth: usize, out: &mut String) {
    let kinds = if depth >= 5 { 5 } else { 7 };
    let kind = rng.below(kinds);
    value_of(rng, kind, depth, out);
}

fn value_of(rng: &mut Rng, kind: usize, depth: usize, out: &mut String) {
    match kind {
        0 => out.push_str("null"),
        1 => {
            let words: [&str; 2] = ["true", "false"];
            let word = *rng.pick(&words);
            out.push_str(word)
        }
        2 => number(rng, out),
        3 | 4 => string(rng, out),
        5 => {
            out.push('[');
            whitespace(rng, out);
            for i in 0..rng.below(6) {
                if i > 0 {
                    out.push(',');
                    whitespace(rng, out);
                }
                value(rng, depth + 1, out);
                whitespace(rng, out);
            }
            out.push(']');
        }
        _ => {
            out.push('{');
            whitespace(rng, out);
            for i in 0..rng.below(9) {
                if i > 0 {
                    out.push(',');
                    whitespace(rng, out);
                }
                // few names, so keys repeat
                if rng.below(4) == 0 {
                    string(rng, out);
                } else {
                    write!(out, "\"{}\"", rng.pick(&["a", "b", "id", "é", "k\\n"])).unwrap();
                }
                whitespace(rng, out);
                out.push(':');
                whitespace(rng, out);
                value(rng, depth + 1, out);
                whitespace(rng, out);
            }
            out.push('}');
        }
    }
}

/// Bytes that sit at the grammar's edges: structure, number parts, the starts of literals, control
/// characters, and UTF-8 lead, continuation and invalid bytes.
const EDGES: &[u8] = b"{}[]:,\"\\ 0123456789-+.eEtrufalsn\t\n\r\x00\x01\x1f\x7f\x80\xbf\xc0\xc3\xe2\xed\xf0\xf4\xff";

/// One to three edits: a byte removed, replaced or inserted.
fn damage(rng: &mut Rng, bytes: &mut Vec<u8>) {
    for _ in 0..1 + rng.below(3) {
        let at = rng.below(bytes.len() + 1);
        let byte = *rng.pick(EDGES);
        match rng.below(3) {
            0 if at < bytes.len() => {
                bytes.remove(at);
            }
            1 if at < bytes.len() => bytes[at] = byte,
            _ => bytes.insert(at, byte),
        }
    }
}

fn shown(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn the_cursor_and_serde_json_accept_the_same_inputs_and_read_the_same_values() {
    let cases = common::cases(100_000);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut read_ok, mut refused, mut out_of_range) = (0usize, 0usize, 0usize);
    let mut buf = String::new();
    for i in 0..cases {
        let mut text = String::new();
        whitespace(&mut rng, &mut text);
        document(&mut rng, &mut text);
        whitespace(&mut rng, &mut text);
        let mut bytes = text.into_bytes();
        let damaged = i % 2 == 1;
        if damaged {
            damage(&mut rng, &mut bytes);
        }
        let theirs = serde_json::from_slice::<Value>(&bytes);
        let ours = Cursor::from_bytes(&bytes)
            .and_then(|mut cursor| read(&mut cursor, &mut buf).and_then(|t| cursor.finish().map(|()| t)));
        assert_eq!(
            ours.is_ok(),
            validate(&bytes).is_ok(),
            "reading and validating disagree on {:?}",
            shown(&bytes)
        );
        match (&ours, &theirs) {
            (Ok(t), Ok(v)) => {
                assert!(same(t, v), "different values from {:?}:\n{t:?}\n{v:?}", shown(&bytes));
                read_ok += 1;
            }
            (Err(_), Err(_)) => refused += 1,
            // a number past a double's range is still JSON: serde_json refuses it, the cursor gives its text
            (Ok(_), Err(e)) if e.to_string().starts_with("number out of range") => out_of_range += 1,
            (Ok(_), Err(e)) => panic!("the cursor accepted what serde_json refused ({e}): {:?}", shown(&bytes)),
            (Err(e), Ok(_)) => panic!("the cursor refused ({e}) what serde_json accepted: {:?}", shown(&bytes)),
        }
        if !damaged {
            assert!(ours.is_ok(), "a generated document is JSON: {:?}", shown(&bytes));
        }
    }
    eprintln!("{cases} cases: {read_ok} read alike, {refused} refused by both, {out_of_range} out of a double's range");
    // the damage has to reach both outcomes, or the test shows little
    assert!(refused > cases / 10, "damage refused too rarely: {refused}");
    assert!(
        read_ok > cases / 2 + cases / 50,
        "damage accepted too rarely: {read_ok}"
    );
}
