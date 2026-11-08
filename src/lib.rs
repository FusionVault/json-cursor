//! Read JSON one value at a time: fast, strict, and with every number exact.
//!
//! A [`Cursor`] walks through a JSON document from start to end. You ask for the next key, string
//! or number, and it hands it to you straight from your bytes, so you can build your own types
//! with no tree in between.
//!
//! ```
//! use json_cursor::Cursor;
//!
//! let mut cursor = Cursor::new(r#"{"name": "Ada", "born": 1815, "languages": ["English"]}"#);
//! let mut buf = String::new(); // used only when a key or string has escapes
//!
//! let mut next = cursor.first_key(&mut buf)?;
//! while let Some(key) = next {
//!     match key {
//!         "name" => assert_eq!(cursor.string(&mut buf)?, "Ada"),
//!         "born" => assert_eq!(cursor.number()?.as_i64(), Some(1815)),
//!         _ => cursor.skip()?, // anything else is checked, then passed over
//!     }
//!     next = cursor.next_key(&mut buf)?;
//! }
//! cursor.finish()?; // only whitespace may follow
//! # Ok::<(), json_cursor::Error>(())
//! ```
//!
//! - [`Cursor`] reads a document one value at a time.
//! - [`Number`] keeps a number's text exactly, and gives its value as an `i64` or an `f64`.
//! - [`validate`] checks a whole document in one call.
//! - [`Error`] says what went wrong, and at which byte.
//!
//! The [README](https://github.com/FusionVault/json-cursor) has a guide, and the benchmarks against
//! other parsers.

#![no_std]

extern crate alloc;

/// Every Rust code block in the README is compiled and run as a doctest.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;

// One part of JSON in each module. `cursor` is the public API, on top of the rest.
mod cursor; // the cursor: reads values one at a time
mod error; // what went wrong, and at which byte
mod number; // numbers: their grammar, their digits, their exact values
mod scan; // the position in the input: whitespace, literals, eight bytes at a time
mod skip; // passing over a value of any kind, checked, on a stack of its own
mod text; // strings: plain runs, escapes, UTF-8

// What the unit tests share.
#[cfg(test)]
mod testing;

pub use cursor::{Cursor, Kind, Value, DEFAULT_DEPTH_LIMIT};
pub use error::{Error, ErrorKind};
pub use number::Number;

/// Checks that `bytes` hold one JSON value, with nothing but whitespace around it.
///
/// It reads them once. Their UTF-8 is checked inside strings, and only where a string has a byte
/// that is not ASCII: outside strings, the grammar admits ASCII alone.
///
/// ```
/// assert!(json_cursor::validate(br#"{"a": [1, 2.5, "x"]}"#).is_ok());
/// assert!(json_cursor::validate(br#"{"a": 1,}"#).is_err());
/// ```
pub fn validate(bytes: &[u8]) -> Result<(), Error> {
    let mut scan = scan::Scan::new(bytes);
    skip::skip::<true>(&mut scan, DEFAULT_DEPTH_LIMIT)?;
    scan.finish()
}
