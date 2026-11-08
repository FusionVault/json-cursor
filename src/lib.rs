#![no_std]
#![doc = include_str!("../README.md")]

extern crate alloc;

mod cursor;
mod error;
mod number;
mod scan;
mod text;

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
    cursor::skip::skip::<true>(&mut scan, DEFAULT_DEPTH_LIMIT)?;
    scan.finish()
}
