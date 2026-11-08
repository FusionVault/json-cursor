//! What went wrong, and where.

use core::fmt;

/// Why the bytes are not JSON, and the byte where the cursor found out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    kind: ErrorKind,
    at: usize,
}

/// Why the bytes are not JSON.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// The input ended inside a value.
    End,
    /// A byte the grammar does not allow here. It names what was expected, such as "`,` or `}`".
    Expected(&'static str),
    /// A byte below 0x20 inside a string. JSON needs it escaped.
    ControlCharacter,
    /// A backslash followed by anything but `" \ / b f n r t u`, or `\u` without four hex digits.
    Escape,
    /// A `\u` escape of half a surrogate pair, with no other half.
    Surrogate,
    /// Bytes that are not UTF-8.
    Utf8,
    /// A number outside JSON's grammar, such as `-`, `1.` or `1e`.
    Number,
    /// Arrays and objects nested deeper than the cursor's depth limit.
    TooDeep,
    /// Bytes after the value, other than whitespace.
    Trailing,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, at: usize) -> Self {
        Error { kind, at }
    }

    /// Why the bytes are not JSON.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// The offset of the byte where the cursor found out.
    pub fn position(&self) -> usize {
        self.at
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.kind {
            ErrorKind::End => "the input ended inside a value",
            ErrorKind::Expected(what) => return write!(f, "expected {what} at byte {}", self.at),
            ErrorKind::ControlCharacter => "a control character in a string, which JSON needs escaped,",
            ErrorKind::Escape => "a bad escape",
            ErrorKind::Surrogate => "half of a surrogate pair",
            ErrorKind::Utf8 => "bytes that are not UTF-8",
            ErrorKind::Number => "a number outside JSON's grammar",
            ErrorKind::TooDeep => "arrays and objects nested deeper than the limit",
            ErrorKind::Trailing => "bytes after the value",
        };
        write!(f, "{what} at byte {}", self.at)
    }
}

impl core::error::Error for Error {}
