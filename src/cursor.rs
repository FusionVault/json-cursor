//! The cursor: a position in the input, moved forward one token at a time.

use alloc::string::String;
use core::str;

use crate::number::{self, Number};
use crate::scan::Scan;
use crate::{skip, text};
use crate::{Error, ErrorKind};

/// What kind of value is next, from its first byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `{`
    Object,
    /// `[`
    Array,
    /// `"`
    String,
    /// `-` or a digit
    Number,
    /// `t` or `f`
    Bool,
    /// `n`
    Null,
}

/// The next value, as [`Cursor::value`] reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value<'s> {
    /// A string, borrowed from the input or decoded into the caller's buffer.
    String(&'s str),
    /// A number: its exact text, and its value as an `i64` or an `f64`.
    Number(Number<'s>),
    /// `true` or `false`.
    Bool(bool),
    /// `null`.
    Null,
    /// An array starts here: read it with [`Cursor::first_item`].
    Array,
    /// An object starts here: read it with [`Cursor::first_key`].
    Object,
}

/// How deep [`Cursor::skip`] goes into arrays and objects, unless set with
/// [`Cursor::with_depth_limit`]: serde_json's limit.
pub const DEFAULT_DEPTH_LIMIT: usize = 128;

/// A strict JSON pull cursor over text.
///
/// Make one from text with [`new`](Self::new), or from bytes with [`from_bytes`](Self::from_bytes),
/// which checks their UTF-8 once, in one pass. Every key, string and number is then a slice of the
/// text, with no further check.
///
/// Read the next value with [`value`](Self::value), or with the call for its kind:
/// [`string`](Self::string), [`number`](Self::number), [`bool`](Self::bool), [`null`](Self::null), an
/// object's keys with [`first_key`](Self::first_key) and [`next_key`](Self::next_key), an array's
/// items with [`first_item`](Self::first_item) and [`next_item`](Self::next_item), or any value with
/// [`skip`](Self::skip). [`peek`](Self::peek) says which kind is next. [`finish`](Self::finish)
/// checks that nothing follows.
///
/// Every byte read is checked against RFC 8259, skipped values included: a call returns an
/// [`Error`] at the first byte that is not JSON. After an error, the cursor's position is
/// unspecified, so start again from the input.
#[derive(Clone, Debug)]
pub struct Cursor<'a> {
    text: &'a str,
    scan: Scan<'a>,
    depth_limit: usize,
}

impl<'a> Cursor<'a> {
    /// A cursor at the start of `text`.
    #[inline]
    pub fn new(text: &'a str) -> Self {
        Cursor {
            text,
            scan: Scan::new(text.as_bytes()),
            depth_limit: DEFAULT_DEPTH_LIMIT,
        }
    }

    /// A cursor at the start of `bytes`, once they are checked as UTF-8, in one pass. `Err` with
    /// [`ErrorKind::Utf8`] at the first byte that is not.
    #[inline]
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Self, Error> {
        str::from_utf8(bytes)
            .map(Self::new)
            .map_err(|e| Error::new(ErrorKind::Utf8, e.valid_up_to()))
    }

    /// How deep [`skip`](Self::skip) goes into arrays and objects before it returns
    /// [`ErrorKind::TooDeep`]. The skip keeps its own stack, so no limit can overflow the call
    /// stack. [`DEFAULT_DEPTH_LIMIT`] when not set.
    #[inline]
    pub fn with_depth_limit(mut self, limit: usize) -> Self {
        self.depth_limit = limit;
        self
    }

    /// The offset of the next byte to read.
    #[inline]
    pub fn position(&self) -> usize {
        self.scan.at
    }

    /// The kind of the next value, after any whitespace. Nothing is consumed.
    #[inline]
    pub fn peek(&mut self) -> Result<Kind, Error> {
        Ok(match self.scan.next_byte()? {
            b'{' => Kind::Object,
            b'[' => Kind::Array,
            b'"' => Kind::String,
            b'-' | b'0'..=b'9' => Kind::Number,
            b't' | b'f' => Kind::Bool,
            b'n' => Kind::Null,
            _ => return Err(self.scan.err(ErrorKind::Expected("a value"))),
        })
    }

    /// The next value, after any whitespace, in one look at its first byte: a string, a number, a
    /// boolean or `null`, read; or the start of an array or an object, not consumed, to be read with
    /// [`first_item`](Self::first_item) or [`first_key`](Self::first_key). A string is borrowed
    /// or decoded as in [`string`](Self::string).
    #[inline]
    pub fn value<'s>(&mut self, buf: &'s mut String) -> Result<Value<'s>, Error>
    where
        'a: 's,
    {
        Ok(match self.scan.next_byte()? {
            b'"' => Value::String(self.string_at(buf)?),
            b'-' | b'0'..=b'9' => Value::Number(self.number_at()?),
            b'{' => Value::Object,
            b'[' => Value::Array,
            b't' => self.scan.literal(b"true", "`true`").map(|()| Value::Bool(true))?,
            b'f' => self.scan.literal(b"false", "`false`").map(|()| Value::Bool(false))?,
            b'n' => self.scan.literal(b"null", "`null`").map(|()| Value::Null)?,
            _ => return Err(self.scan.err(ErrorKind::Expected("a value"))),
        })
    }

    /// Enters the object at the cursor and reads its first key and colon, or `None` when the object
    /// is empty. Read the key's value next. The key is borrowed from the input when it has no
    /// escapes, else decoded into `buf`, which it then borrows.
    #[inline]
    pub fn first_key<'s>(&mut self, buf: &'s mut String) -> Result<Option<&'s str>, Error>
    where
        'a: 's,
    {
        self.scan.expect(b'{', "`{`")?;
        match self.scan.next_byte()? {
            b'}' => {
                self.scan.at += 1;
                Ok(None)
            }
            b'"' => self.key(buf).map(Some),
            _ => Err(self.scan.err(ErrorKind::Expected("a key or `}`"))),
        }
    }

    /// After a value in an object: the next key and colon, or `None` at the object's end, which is
    /// consumed. The key is borrowed as in [`first_key`](Self::first_key).
    #[inline]
    pub fn next_key<'s>(&mut self, buf: &'s mut String) -> Result<Option<&'s str>, Error>
    where
        'a: 's,
    {
        match self.scan.next_byte()? {
            b',' => {
                self.scan.at += 1;
                match self.scan.next_byte()? {
                    b'"' => self.key(buf).map(Some),
                    _ => Err(self.scan.err(ErrorKind::Expected("a key"))),
                }
            }
            b'}' => {
                self.scan.at += 1;
                Ok(None)
            }
            _ => Err(self.scan.err(ErrorKind::Expected("`,` or `}`"))),
        }
    }

    /// Enters the array at the cursor: `true` when it has a first item, which is read next, and
    /// `false` when it is empty.
    #[inline]
    pub fn first_item(&mut self) -> Result<bool, Error> {
        self.scan.expect(b'[', "`[`")?;
        if self.scan.next_byte()? == b']' {
            self.scan.at += 1;
            Ok(false)
        } else {
            Ok(true)
        }
    }

    /// After an item in an array: `true` when another follows, which is read next, and `false` at
    /// the array's end, which is consumed.
    #[inline]
    pub fn next_item(&mut self) -> Result<bool, Error> {
        match self.scan.next_byte()? {
            b',' => {
                self.scan.at += 1;
                Ok(true)
            }
            b']' => {
                self.scan.at += 1;
                Ok(false)
            }
            _ => Err(self.scan.err(ErrorKind::Expected("`,` or `]`"))),
        }
    }

    /// The string at the cursor: borrowed from the input when it has no escapes, else decoded into
    /// `buf`, which it then borrows.
    #[inline]
    pub fn string<'s>(&mut self, buf: &'s mut String) -> Result<&'s str, Error>
    where
        'a: 's,
    {
        if self.scan.next_byte()? != b'"' {
            return Err(self.scan.err(ErrorKind::Expected("a string")));
        }
        self.string_at(buf)
    }

    /// The number at the cursor: its exact text, and its value as an `i64` or an `f64`. Its grammar
    /// is checked; its size is not limited, since the text keeps every digit.
    #[inline]
    pub fn number(&mut self) -> Result<Number<'a>, Error> {
        match self.scan.next_byte()? {
            b'-' | b'0'..=b'9' => self.number_at(),
            _ => Err(self.scan.err(ErrorKind::Expected("a number"))),
        }
    }

    /// The `true` or `false` at the cursor.
    #[inline]
    pub fn bool(&mut self) -> Result<bool, Error> {
        match self.scan.next_byte()? {
            b't' => self.scan.literal(b"true", "`true`").map(|()| true),
            b'f' => self.scan.literal(b"false", "`false`").map(|()| false),
            _ => Err(self.scan.err(ErrorKind::Expected("`true` or `false`"))),
        }
    }

    /// The `null` at the cursor.
    #[inline]
    pub fn null(&mut self) -> Result<(), Error> {
        if self.scan.next_byte()? != b'n' {
            return Err(self.scan.err(ErrorKind::Expected("`null`")));
        }
        self.scan.literal(b"null", "`null`")
    }

    /// Reads past the value at the cursor, of any kind, checking every byte of it: its grammar, its
    /// strings' escapes, and its depth. Numbers are checked but not read.
    #[inline]
    pub fn skip(&mut self) -> Result<(), Error> {
        skip::skip::<false>(&mut self.scan, self.depth_limit)
    }

    /// Checks that only whitespace follows.
    #[inline]
    pub fn finish(&mut self) -> Result<(), Error> {
        self.scan.finish()
    }

    /// At a number's first byte: the number. Inlined, so a value is not returned through memory.
    #[inline(always)]
    fn number_at(&mut self) -> Result<Number<'a>, Error> {
        let start = self.scan.at;
        let (end, parts) = number::read::<true>(self.scan.bytes, start)?;
        self.scan.at = end;
        Ok(Number::new(self.text, start, end, parts))
    }

    /// At a string's opening quote: the string, borrowed or decoded into `buf`.
    #[inline(always)]
    fn string_at<'s>(&mut self, buf: &'s mut String) -> Result<&'s str, Error>
    where
        'a: 's,
    {
        let (end, borrowed) = text::read(self.text, self.scan.at, buf)?;
        self.scan.at = end;
        Ok(match borrowed {
            Some(text) => text,
            None => buf.as_str(),
        })
    }

    /// At a key's opening quote: the key, then its colon.
    #[inline]
    fn key<'s>(&mut self, buf: &'s mut String) -> Result<&'s str, Error>
    where
        'a: 's,
    {
        let key = self.string_at(buf)?;
        self.scan.expect(b':', "`:`")?;
        Ok(key)
    }
}
