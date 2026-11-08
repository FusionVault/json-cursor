//! A position in the input's bytes, and the reads every value starts with.

use crate::{Error, ErrorKind};

/// The input's bytes, and the offset of the next one to read.
#[derive(Clone, Debug)]
pub(crate) struct Scan<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) at: usize,
}

impl<'a> Scan<'a> {
    #[inline]
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Scan { bytes, at: 0 }
    }

    #[inline]
    pub(crate) fn err(&self, kind: ErrorKind) -> Error {
        Error::new(kind, self.at)
    }

    #[inline]
    pub(crate) fn whitespace(&mut self) {
        self.at = whitespace(self.bytes, self.at);
    }

    /// The next byte after any whitespace, not consumed.
    #[inline]
    pub(crate) fn next_byte(&mut self) -> Result<u8, Error> {
        self.whitespace();
        self.bytes.get(self.at).copied().ok_or_else(|| self.err(ErrorKind::End))
    }

    #[inline]
    pub(crate) fn expect(&mut self, byte: u8, what: &'static str) -> Result<(), Error> {
        if self.next_byte()? == byte {
            self.at += 1;
            Ok(())
        } else {
            Err(self.err(ErrorKind::Expected(what)))
        }
    }

    /// The literal `word` at the cursor, named `what` in an error.
    #[inline]
    pub(crate) fn literal(&mut self, word: &[u8], what: &'static str) -> Result<(), Error> {
        self.at = literal(self.bytes, self.at, word, what)?;
        Ok(())
    }

    /// Checks that only whitespace follows.
    #[inline]
    pub(crate) fn finish(&mut self) -> Result<(), Error> {
        self.whitespace();
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(self.err(ErrorKind::Trailing))
        }
    }
}

/// From `at`, the offset of the first byte that is not whitespace.
#[inline]
pub(crate) fn whitespace(bytes: &[u8], mut at: usize) -> usize {
    while let Some(b' ' | b'\t' | b'\n' | b'\r') = bytes.get(at) {
        at += 1;
    }
    at
}

/// The literal `word` at `at`, named `what` in an error: the offset after it.
#[inline]
pub(crate) fn literal(bytes: &[u8], at: usize, word: &[u8], what: &'static str) -> Result<usize, Error> {
    let rest = bytes.get(at..).unwrap_or_default();
    if rest.starts_with(word) {
        Ok(at + word.len())
    } else if word.starts_with(rest) {
        Err(Error::new(ErrorKind::End, bytes.len()))
    } else {
        Err(Error::new(ErrorKind::Expected(what), at))
    }
}

/// The byte at `at`, or the error for the input's end there.
#[inline(always)]
pub(crate) fn byte(bytes: &[u8], at: usize) -> Result<u8, Error> {
    bytes.get(at).copied().ok_or(Error::new(ErrorKind::End, at))
}
