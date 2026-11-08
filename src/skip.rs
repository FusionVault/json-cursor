//! Skipping a value of any kind, checked byte by byte, on a stack of its own.

use alloc::vec::Vec;

use crate::scan::{self, byte, Scan};
use crate::{number, text, Error, ErrorKind};

/// Reads past the value at `s`, of any kind, checking every byte of it: its grammar, its strings'
/// escapes, and its depth, up to `depth_limit`. Numbers are checked but not read. With `UTF8`, the
/// input's UTF-8 was not checked before, so a string's bytes that are not ASCII are checked as they
/// pass; outside strings, the grammar admits ASCII alone.
pub(crate) fn skip<const UTF8: bool>(s: &mut Scan<'_>, depth_limit: usize) -> Result<(), Error> {
    s.at = skip_from::<UTF8>(s.bytes, s.at, depth_limit)?;
    Ok(())
}

/// The skip from `at`: the offset after the value. The position and the stack's word are
/// locals, so they stay in registers instead of going to memory at every byte.
fn skip_from<const UTF8: bool>(bytes: &[u8], mut at: usize, depth_limit: usize) -> Result<usize, Error> {
    let mut open = Open::new(depth_limit);
    let mut deep = Vec::new();
    loop {
        at = scan::whitespace(bytes, at);
        match byte(bytes, at)? {
            b'{' => {
                open.enter(true, depth_limit, at, &mut deep)?;
                at = scan::whitespace(bytes, at + 1);
                match byte(bytes, at)? {
                    b'}' => {
                        at += 1;
                        open.close(depth_limit, &mut deep);
                    }
                    b'"' => {
                        at = key::<UTF8>(bytes, at)?;
                        continue;
                    }
                    _ => return Err(Error::new(ErrorKind::Expected("a key or `}`"), at)),
                }
            }
            b'[' => {
                open.enter(false, depth_limit, at, &mut deep)?;
                at = scan::whitespace(bytes, at + 1);
                if byte(bytes, at)? != b']' {
                    continue;
                }
                at += 1;
                open.close(depth_limit, &mut deep);
            }
            b'"' => at = text::skip::<UTF8>(bytes, at)?,
            b'-' | b'0'..=b'9' => at = number::read::<false>(bytes, at)?.0,
            b't' => at = scan::literal(bytes, at, b"true", "`true`")?,
            b'f' => at = scan::literal(bytes, at, b"false", "`false`")?,
            b'n' => at = scan::literal(bytes, at, b"null", "`null`")?,
            _ => return Err(Error::new(ErrorKind::Expected("a value"), at)),
        }
        // a value ended: close what it ends, or step to the next value
        loop {
            if open.top() {
                return Ok(at);
            }
            let object = open.object();
            at = scan::whitespace(bytes, at);
            match byte(bytes, at)? {
                b',' => {
                    at += 1;
                    if object {
                        at = scan::whitespace(bytes, at);
                        if byte(bytes, at)? != b'"' {
                            return Err(Error::new(ErrorKind::Expected("a key"), at));
                        }
                        at = key::<UTF8>(bytes, at)?;
                    }
                    break;
                }
                b'}' if object => {
                    at += 1;
                    open.close(depth_limit, &mut deep);
                }
                b']' if !object => {
                    at += 1;
                    open.close(depth_limit, &mut deep);
                }
                _ => {
                    let want = if object { "`,` or `}`" } else { "`,` or `]`" };
                    return Err(Error::new(ErrorKind::Expected(want), at));
                }
            }
        }
    }
}

/// At a key's opening quote: the offset after its colon.
#[inline(always)]
fn key<const UTF8: bool>(bytes: &[u8], at: usize) -> Result<usize, Error> {
    let at = scan::whitespace(bytes, text::skip::<UTF8>(bytes, at)?);
    if byte(bytes, at)? != b':' {
        return Err(Error::new(ErrorKind::Expected("`:`"), at));
    }
    Ok(at + 1)
}

/// The arrays and objects a skip is inside: a bit each, set for an object, the innermost lowest,
/// above a sentinel bit, so `word` is 1 at the top level. A word holds 63 levels; past them, full
/// words wait in `deep`, which a skip allocates only that deep and touches out of line. The depth
/// limit and the word's size are one bound, `room`, so entering costs one comparison, and the word
/// and its room stay in registers.
struct Open {
    word: u64,
    /// The word reaches this once it holds as many levels as it may: the sentinel's place, at 63
    /// levels or at the depth limit, whichever comes first.
    room: u64,
}

impl Open {
    fn new(limit: usize) -> Self {
        Open {
            word: 1,
            room: room(limit, 0),
        }
    }

    /// Opens an array or an object, at `at`, unless that is deeper than `limit`.
    #[inline(always)]
    fn enter(&mut self, object: bool, limit: usize, at: usize, deep: &mut Vec<u64>) -> Result<(), Error> {
        if self.word >= self.room {
            *self = deeper(deep, self.word, limit).ok_or(Error::new(ErrorKind::TooDeep, at))?;
        }
        self.word = self.word << 1 | u64::from(object);
        Ok(())
    }

    /// Closes the innermost.
    #[inline(always)]
    fn close(&mut self, limit: usize, deep: &mut Vec<u64>) {
        self.word >>= 1;
        if self.word == 1 && !deep.is_empty() {
            *self = shallower(deep, limit);
        }
    }

    /// Whether the skip is at the top level, inside nothing.
    #[inline(always)]
    fn top(&self) -> bool {
        self.word == 1
    }

    /// Whether the innermost is an object. At the top level, the sentinel answers, so ask only inside.
    #[inline(always)]
    fn object(&self) -> bool {
        self.word & 1 == 1
    }
}

/// A word's room, with `spilled` full words below it: 63 levels, or fewer where the limit comes first.
fn room(limit: usize, spilled: usize) -> u64 {
    1 << limit.saturating_sub(63 * spilled).min(63)
}

/// The word is at its room: past the limit, `None`; else it is full, so it goes to `deep`, and a
/// fresh one takes its place.
#[cold]
#[inline(never)]
fn deeper(deep: &mut Vec<u64>, word: u64, limit: usize) -> Option<Open> {
    let levels = 63 - word.leading_zeros() as usize;
    if levels + 63 * deep.len() >= limit {
        return None;
    }
    deep.push(word);
    Some(Open {
        word: 1,
        room: room(limit, deep.len()),
    })
}

/// The word emptied with full words below it: the last of them comes back.
#[cold]
#[inline(never)]
fn shallower(deep: &mut Vec<u64>, limit: usize) -> Open {
    let word = deep.pop().unwrap_or(1);
    Open {
        word,
        room: room(limit, deep.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::Open;
    use crate::testing::Rng;
    use alloc::vec::Vec;

    /// The bit-stack against a `Vec<bool>`, up past 63 levels and two words beyond, and back down.
    #[test]
    fn the_open_stack_keeps_each_level_past_its_first_word() {
        let mut open = Open::new(usize::MAX);
        let mut deep = Vec::new();
        let mut reference: Vec<bool> = Vec::new();
        let mut rng = Rng(0x2545_F491_4F6C_DD1D);
        for step in 0..20_000 {
            let r = rng.next();
            // climb for a while, then mostly descend, so every depth to about 200 is crossed
            let up = (step / 400) % 2 == 0;
            if reference.is_empty() || (up && r % 4 != 0) || (!up && r % 4 == 0) {
                let object = r & 8 == 0;
                open.enter(object, usize::MAX, 0, &mut deep).unwrap();
                reference.push(object);
            } else {
                reference.pop();
                open.close(usize::MAX, &mut deep);
            }
            assert_eq!(open.top(), reference.is_empty(), "at depth {}", reference.len());
            if let Some(&object) = reference.last() {
                assert_eq!(open.object(), object, "at depth {}", reference.len());
            }
        }
        assert!(deep.capacity() > 0, "the test never went past one word");
    }

    /// Exactly `limit` levels open, at every limit near a word's edge, and the next is refused.
    #[test]
    fn the_open_stack_stops_at_the_limit() {
        for limit in [0, 1, 2, 62, 63, 64, 65, 125, 126, 127, 128, 189, 200] {
            let mut open = Open::new(limit);
            let mut deep = Vec::new();
            let mut levels = 0;
            while open.enter(levels % 3 == 0, limit, levels, &mut deep).is_ok() {
                levels += 1;
                assert!(levels <= limit, "past the limit of {limit}");
            }
            assert_eq!(levels, limit);
            for _ in 0..limit {
                assert!(!open.top());
                open.close(limit, &mut deep);
            }
            assert!(open.top(), "limit {limit}");
        }
    }
}
