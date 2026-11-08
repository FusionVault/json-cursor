//! Strings: where a run of plain bytes ends, a word or two at a time, and what each escape stands for.

use alloc::string::String;

use crate::{Error, ErrorKind};

/// At a string's opening quote, at `at` in `text`: reads to its closing quote, checking every byte.
/// Gives the offset after the closing quote, and the string borrowed from `text` when it has no
/// escapes, else `None`, once decoded into `out`.
#[inline(always)]
pub(crate) fn scan<'a>(text: &'a str, at: usize, out: &mut String) -> Result<(usize, Option<&'a str>), Error> {
    let end = plain_run(text.as_bytes(), at + 1);
    if text.as_bytes().get(end) == Some(&b'"') {
        return Ok((end + 1, Some(slice(text, at + 1, end)?)));
    }
    decode(text, at + 1, end, out).map(|end| (end, None))
}

/// The rest of a string whose first run, from `run` to `end`, does not end it: its escapes decoded
/// into `out`, with the offset after its closing quote. Out of line, so the path of a plain string
/// stays small where it is inlined.
#[inline(never)]
fn decode(text: &str, mut run: usize, mut end: usize, out: &mut String) -> Result<usize, Error> {
    let bytes = text.as_bytes();
    out.clear();
    loop {
        match bytes.get(end) {
            None => return Err(Error::new(ErrorKind::End, end)),
            Some(b'"') => {
                out.push_str(slice(text, run, end)?);
                return Ok(end + 1);
            }
            Some(b'\\') => {
                let (c, next) = escape(bytes, end)?;
                out.push_str(slice(text, run, end)?);
                out.push(c);
                run = next;
                end = plain_run(bytes, run);
            }
            Some(_) => return Err(Error::new(ErrorKind::ControlCharacter, end)),
        }
    }
}

/// At a string's opening quote, at `at`: the offset after its closing quote, checking its grammar
/// and escapes. With `UTF8`, the input's UTF-8 was not checked before, so a run with a byte that is
/// not ASCII is checked here; a multi-byte character never spans an escape or a quote, which are
/// ASCII, so each run is whole.
#[inline(always)]
pub(crate) fn skip<const UTF8: bool>(bytes: &[u8], at: usize) -> Result<usize, Error> {
    let mut run = at + 1;
    loop {
        let (end, ascii) = plain_run_ascii::<UTF8>(bytes, run);
        if UTF8 && !ascii {
            utf8(bytes, run, end)?;
        }
        match bytes.get(end) {
            None => return Err(Error::new(ErrorKind::End, end)),
            Some(b'"') => return Ok(end + 1),
            Some(b'\\') => {
                // a one-letter escape is checked here; a `u` escape, and any error, out of line
                run = match bytes.get(end + 1) {
                    Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => end + 2,
                    _ => escape(bytes, end)?.1,
                }
            }
            Some(_) => return Err(Error::new(ErrorKind::ControlCharacter, end)),
        }
    }
}

/// Checks that the bytes from `start` to `end` are UTF-8: out of line, since most runs are ASCII.
#[inline(never)]
fn utf8(bytes: &[u8], start: usize, end: usize) -> Result<(), Error> {
    let run = bytes.get(start..end).unwrap_or_default();
    match core::str::from_utf8(run) {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::new(ErrorKind::Utf8, start + e.valid_up_to())),
    }
}

/// The text from `start` to `end`, both offsets of ASCII bytes, so always char boundaries.
#[inline]
pub(crate) fn slice(text: &str, start: usize, end: usize) -> Result<&str, Error> {
    text.get(start..end).ok_or(Error::new(ErrorKind::Utf8, start))
}

/// At the backslash at `at`: the character its escape stands for, and the offset after it.
fn escape(bytes: &[u8], at: usize) -> Result<(char, usize), Error> {
    let c = match bytes.get(at + 1) {
        None => return Err(Error::new(ErrorKind::End, bytes.len())),
        Some(b'"') => '"',
        Some(b'\\') => '\\',
        Some(b'/') => '/',
        Some(b'b') => '\u{8}',
        Some(b'f') => '\u{c}',
        Some(b'n') => '\n',
        Some(b'r') => '\r',
        Some(b't') => '\t',
        Some(b'u') => {
            let mut next = at + 6;
            let code = match hex4(bytes, at + 2)? {
                high @ 0xD800..=0xDBFF => {
                    // a high surrogate: its low half must follow
                    if bytes.get(next..next + 2) != Some(b"\\u") {
                        return Err(Error::new(ErrorKind::Surrogate, at));
                    }
                    let low = hex4(bytes, next + 2)?;
                    if !(0xDC00..=0xDFFF).contains(&low) {
                        return Err(Error::new(ErrorKind::Surrogate, at));
                    }
                    next += 6;
                    0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00)
                }
                0xDC00..=0xDFFF => return Err(Error::new(ErrorKind::Surrogate, at)),
                code => code,
            };
            let c = char::from_u32(code).ok_or(Error::new(ErrorKind::Escape, at))?;
            return Ok((c, next));
        }
        Some(_) => return Err(Error::new(ErrorKind::Escape, at)),
    };
    Ok((c, at + 2))
}

/// The four hex digits at `at`, as a number. A byte that is not one is a bad escape, even when the
/// input ends before the fourth.
fn hex4(bytes: &[u8], at: usize) -> Result<u32, Error> {
    let mut code = 0;
    for i in 0..4 {
        let v = match bytes.get(at + i) {
            None => return Err(Error::new(ErrorKind::End, bytes.len())),
            Some(&d @ b'0'..=b'9') => d - b'0',
            Some(&d @ b'a'..=b'f') => d - b'a' + 10,
            Some(&d @ b'A'..=b'F') => d - b'A' + 10,
            Some(_) => return Err(Error::new(ErrorKind::Escape, at + i)),
        };
        code = code << 4 | u32::from(v);
    }
    Ok(code)
}

/// From `at`, the offset of the byte that ends a run of plain string bytes: a quote, a backslash, a
/// control character, or the input's end. A word at a time, then two at a time for a long run.
#[inline(always)]
pub(crate) fn plain_run(bytes: &[u8], at: usize) -> usize {
    plain_run_ascii::<false>(bytes, at).0
}

/// [`plain_run`], and with `ASCII`, whether every byte of the run is ASCII; without it, `true`.
#[inline(always)]
pub(crate) fn plain_run_ascii<const ASCII: bool>(bytes: &[u8], mut at: usize) -> (usize, bool) {
    // the high bits of the run's bytes, all of them clear while it is ASCII
    let mut high = 0;
    // the first word alone, since most keys and many strings end in it
    if let Some(w) = bytes.get(at..at + 8) {
        let w = word(w);
        let e = ends(w);
        if e != 0 {
            return (at + end_in(e), !ASCII || before(w, e) == 0);
        }
        high |= w;
        at += 8;
        // then two words at a time, for the long ones
        while let Some(w) = bytes.get(at..at + 16) {
            let (a, b) = (word(&w[..8]), word(&w[8..]));
            let (ea, eb) = (ends(a), ends(b));
            if ea | eb != 0 {
                let (w, e, at) = if ea != 0 { (a, ea, at) } else { (b, eb, at + 8) };
                high |= if ea != 0 { 0 } else { a };
                return (at + end_in(e), !ASCII || (high & HIGH | before(w, e)) == 0);
            }
            high |= a | b;
            at += 16;
        }
    }
    while let Some(w) = bytes.get(at..at + 8) {
        let w = word(w);
        let e = ends(w);
        if e != 0 {
            return (at + end_in(e), !ASCII || (high & HIGH | before(w, e)) == 0);
        }
        high |= w;
        at += 8;
    }
    let mut high = high & HIGH;
    while let Some(&b) = bytes.get(at) {
        if b == b'"' || b == b'\\' || b < 0x20 {
            break;
        }
        high |= u64::from(b & 0x80);
        at += 1;
    }
    (at, !ASCII || high == 0)
}

const ONES: u64 = 0x0101_0101_0101_0101;
const HIGH: u64 = 0x8080_8080_8080_8080;

/// Eight bytes, the first in the lowest.
#[inline(always)]
fn word(bytes: &[u8]) -> u64 {
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// The high bit of each byte of `w` that is a quote, a backslash or below 0x20: exact up to the
/// first such byte, which is all that is read, since a borrow only runs upwards.
#[inline(always)]
fn ends(w: u64) -> u64 {
    let quote = w ^ (ONES * u64::from(b'"'));
    let backslash = w ^ (ONES * u64::from(b'\\'));
    (quote.wrapping_sub(ONES) & !quote | backslash.wrapping_sub(ONES) & !backslash | w.wrapping_sub(ONES * 0x20) & !w)
        & HIGH
}

/// The offset in its word of the first end that `ends` found.
#[inline(always)]
fn end_in(e: u64) -> usize {
    (e.trailing_zeros() / 8) as usize
}

/// The high bits of the bytes of `w` before its first end.
#[inline(always)]
fn before(w: u64, e: u64) -> u64 {
    w & HIGH & (e & e.wrapping_neg()).wrapping_sub(1)
}

#[cfg(test)]
mod tests {
    use super::{plain_run, plain_run_ascii};
    use alloc::vec;

    /// A byte at a time: what `plain_run` must find.
    fn reference(bytes: &[u8], mut at: usize) -> usize {
        while let Some(&b) = bytes.get(at) {
            if b == b'"' || b == b'\\' || b < 0x20 {
                break;
            }
            at += 1;
        }
        at
    }

    /// Every end byte at every offset of runs up to 40 bytes long, from several starts, between
    /// fillers next to the values the trick tests for, with bytes after the end it could flag falsely.
    #[test]
    fn a_plain_run_ends_where_the_byte_loop_says() {
        let ends = [b'"', b'\\', 0x00, 0x01, 0x1f];
        let fillers = [b'a', 0x20, 0x21, 0x23, 0x5b, 0x5d, 0x7f, 0x80, 0xc3, 0xff];
        for len in 0..40 {
            for &end in &ends {
                for pos in 0..=len {
                    for &filler in &fillers {
                        for &other in &fillers {
                            let mut b = vec![filler; len];
                            if pos < len {
                                b[pos] = end;
                            }
                            if pos > 0 {
                                b[pos - 1] = other;
                            }
                            b.extend_from_slice(b" \"\x00\x1f\\");
                            for at in 0..=pos.min(3) {
                                let end = reference(&b, at);
                                assert_eq!(plain_run(&b, at), end, "{b:?} from {at}");
                                let ascii = b[at..end].is_ascii();
                                assert_eq!(plain_run_ascii::<true>(&b, at), (end, ascii), "{b:?} from {at}");
                            }
                        }
                    }
                }
            }
        }
    }
}
