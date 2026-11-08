//! Numbers: their grammar, read eight digits at a time, and the values they hold exactly.

use crate::{Error, ErrorKind};

/// A number as the cursor read it.
///
/// [`text`](Self::text) is the number exactly as written. [`as_i64`](Self::as_i64) and
/// [`as_f64`](Self::as_f64) give its value exactly, from the digits the cursor read while it
/// checked the number's grammar, so most numbers need no second pass over their text.
#[derive(Clone, Copy)]
pub struct Number<'a> {
    /// The input, and where the number lies in it: sliced only when the text is asked for.
    input: &'a str,
    start: usize,
    end: usize,
    negative: bool,
    /// The digits with the point taken out: exact when `exact`, with at most 19 of them.
    digits: u64,
    exact: bool,
    /// The power of ten the digits are scaled by: the exponent, less the digits after the point.
    scale: i64,
    /// Written with digits only: no point, no exponent.
    integer: bool,
}

/// What a number's digits say, gathered as they are read.
#[derive(Clone, Copy, Default)]
pub(crate) struct Parts {
    negative: bool,
    digits: u64,
    count: usize,
    scale: i64,
    integer: bool,
}

/// The largest integer a double holds exactly, along with every integer below it.
const EXACT: u64 = 1 << 53;

/// Powers of ten a double holds exactly.
const POW10: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20,
    1e21, 1e22,
];

/// Powers of ten a `u64` holds.
const POW10_U64: [u64; 20] = [
    1,
    10,
    100,
    1_000,
    10_000,
    100_000,
    1_000_000,
    10_000_000,
    100_000_000,
    1_000_000_000,
    10_000_000_000,
    100_000_000_000,
    1_000_000_000_000,
    10_000_000_000_000,
    100_000_000_000_000,
    1_000_000_000_000_000,
    10_000_000_000_000_000,
    100_000_000_000_000_000,
    1_000_000_000_000_000_000,
    10_000_000_000_000_000_000,
];

impl<'a> Number<'a> {
    #[inline]
    pub(crate) fn new(input: &'a str, start: usize, end: usize, p: Parts) -> Self {
        Number {
            input,
            start,
            end,
            negative: p.negative,
            digits: p.digits,
            exact: p.count <= 19,
            scale: p.scale,
            integer: p.integer,
        }
    }

    /// The number exactly as written: `12.50` stays `12.50`, and a 30-digit integer keeps every digit.
    #[inline]
    pub fn text(&self) -> &'a str {
        // both ends are ASCII bytes, so char boundaries: the slice always exists
        self.input.get(self.start..self.end).unwrap_or_default()
    }

    /// Whether it is written with digits only, with no point and no exponent: `7` is, `7.0` and
    /// `7e0` are not.
    #[inline]
    pub fn is_integer(&self) -> bool {
        self.integer
    }

    /// The number as an `i64`, exactly, when it is a whole number that fits, however it is written:
    /// `7`, `7.0`, `700e-2` and `1e3` all are, and `-0` is `0`. `1.5` and `9223372036854775808`
    /// are `None`. No float is used on the way: a fraction is never cut, nor a digit rounded.
    #[inline]
    pub fn as_i64(&self) -> Option<i64> {
        if !self.exact {
            return whole_from_text(self.text());
        }
        let digits = self.digits;
        let magnitude = if digits == 0 {
            0
        } else if self.scale >= 0 {
            digits.checked_mul(*POW10_U64.get(self.scale as usize)?)?
        } else {
            // a whole number when the digits after the point are all zeros
            let p = *POW10_U64.get(self.scale.unsigned_abs() as usize)?;
            if digits % p != 0 {
                return None;
            }
            digits / p
        };
        signed(self.negative, magnitude)
    }

    /// The nearest `f64`, correctly rounded, exactly as Rust's own parser gives it: `1e400` is
    /// infinite, and `-0` is negative zero.
    ///
    /// Most numbers take one multiplication or division, with no second pass. Digits that fit 2^53
    /// and a power of ten up to 10^22 are both exact in a double, so one rounding gives the nearest
    /// (Clinger's fast path). A larger power still qualifies when part of it moves into the digits
    /// and they still fit, as `123e30` is `123000000 × 10^22`. The rest go through `str::parse`.
    #[inline]
    pub fn as_f64(&self) -> f64 {
        if self.exact {
            let digits = self.digits;
            let fast = if digits > EXACT {
                None
            } else if (-22..=0).contains(&self.scale) {
                Some(digits as f64 / POW10[self.scale.unsigned_abs() as usize])
            } else if (0..=22).contains(&self.scale) {
                Some(digits as f64 * POW10[self.scale as usize])
            } else if (23..=22 + 15).contains(&self.scale) {
                digits
                    .checked_mul(POW10_U64[self.scale as usize - 22])
                    .filter(|&m| m <= EXACT)
                    .map(|m| m as f64 * 1e22)
            } else {
                None
            };
            if let Some(v) = fast {
                return if self.negative { -v } else { v };
            }
        }
        parse(self.text())
    }
}

/// Two numbers are equal when they are written the same.
impl PartialEq for Number<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text()
    }
}

impl Eq for Number<'_> {}

impl core::fmt::Debug for Number<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("Number").field(&self.text()).finish()
    }
}

/// A number past the fast paths, through Rust's own parser, which takes JSON's grammar: kept out
/// of line, so the fast paths stay small where they are inlined.
#[cold]
#[inline(never)]
fn parse(text: &str) -> f64 {
    text.parse().unwrap_or(f64::NAN)
}

/// A magnitude with its sign, when it fits an `i64`, down to `i64::MIN`.
#[inline]
fn signed(negative: bool, magnitude: u64) -> Option<i64> {
    if negative {
        (magnitude <= 1 << 63).then(|| 0u64.wrapping_sub(magnitude) as i64)
    } else {
        i64::try_from(magnitude).ok()
    }
}

/// The exact whole value of a number written with more than 19 digits, such as
/// `1000000000000000000000e-3`: zeros at either end of its digits count only for the scale.
#[cold]
#[inline(never)]
fn whole_from_text(text: &str) -> Option<i64> {
    let b = text.as_bytes();
    let negative = b.first() == Some(&b'-');
    let body = &b[usize::from(negative)..];
    let (mantissa, exponent) = match body.iter().position(|&c| c == b'e' || c == b'E') {
        Some(e) => (&body[..e], &body[e + 1..]),
        None => (body, &b""[..]),
    };
    let exponent = match exponent {
        [b'-', d @ ..] => -saturating_digits(d),
        [b'+', d @ ..] | d => saturating_digits(d),
    };
    let (int, frac) = match mantissa.iter().position(|&c| c == b'.') {
        Some(p) => (&mantissa[..p], &mantissa[p + 1..]),
        None => (mantissa, &b""[..]),
    };
    let digit = |i: usize| if i < int.len() { int[i] } else { frac[i - int.len()] };
    let len = int.len() + frac.len();
    let Some(first) = (0..len).find(|&i| digit(i) != b'0') else {
        return Some(0);
    };
    let last = (0..len).rev().find(|&i| digit(i) != b'0')?;
    if last - first >= 19 {
        return None;
    }
    // the power of ten of the last digit that is not a zero
    let scale = exponent
        .saturating_sub(frac.len() as i64)
        .saturating_add((len - 1 - last) as i64);
    let mut magnitude: u64 = 0;
    for i in first..=last {
        magnitude = magnitude * 10 + u64::from(digit(i) - b'0');
    }
    signed(
        negative,
        // a negative scale leaves a fraction, so no whole value
        magnitude.checked_mul(*POW10_U64.get(usize::try_from(scale).ok()?)?)?,
    )
}

fn saturating_digits(d: &[u8]) -> i64 {
    d.iter()
        .fold(0i64, |v, &c| v.saturating_mul(10).saturating_add(i64::from(c - b'0')))
}

/// At a number's first byte, `-` or a digit, at `at`: reads it, checking its grammar, and gives
/// the offset after it. With `VALUE`, also its parts, gathered from the digits as they are read.
#[inline(always)]
pub(crate) fn scan<const VALUE: bool>(bytes: &[u8], start: usize) -> Result<(usize, Parts), Error> {
    let bad = Error::new(ErrorKind::Number, start);
    let mut p = Parts {
        integer: true,
        ..Parts::default()
    };
    let mut at = start;
    if bytes.get(at) == Some(&b'-') {
        p.negative = true;
        at += 1;
    }
    match bytes.get(at) {
        Some(b'0') => {
            at += 1;
            p.count = 1;
        }
        Some(b'1'..=b'9') => {
            let end = digits::<VALUE>(bytes, at, &mut p.digits);
            p.count = end - at;
            at = end;
        }
        _ => return Err(bad),
    }
    let mut fraction = 0;
    if bytes.get(at) == Some(&b'.') {
        at += 1;
        let end = digits::<VALUE>(bytes, at, &mut p.digits);
        fraction = end - at;
        if fraction == 0 {
            return Err(bad);
        }
        at = end;
        p.count += fraction;
        p.integer = false;
    }
    let mut exponent: i64 = 0;
    if let Some(b'e' | b'E') = bytes.get(at) {
        at += 1;
        p.integer = false;
        let down = match bytes.get(at) {
            Some(b'-') => {
                at += 1;
                true
            }
            Some(b'+') => {
                at += 1;
                false
            }
            _ => false,
        };
        let from = at;
        while let Some(&d @ b'0'..=b'9') = bytes.get(at) {
            if VALUE {
                exponent = exponent.saturating_mul(10).saturating_add(i64::from(d - b'0'));
            }
            at += 1;
        }
        if at == from {
            return Err(bad);
        }
        if down {
            exponent = -exponent;
        }
    }
    p.scale = exponent.saturating_sub(fraction as i64);
    // a number ends where its grammar does: in `01` it is `0`, and the next read refuses the `1`
    Ok((at, p))
}

const ZEROS: u64 = 0x3030_3030_3030_3030;

/// From `at`, the run of ASCII digits: its end, and with `VALUE`, the digits added onto `value`,
/// exact while it holds at most 19. Eight at a time while all eight are digits; with no value to
/// gather, a word's leading digits are counted at once.
#[inline(always)]
fn digits<const VALUE: bool>(bytes: &[u8], mut at: usize, value: &mut u64) -> usize {
    while let Some(w) = bytes.get(at..at + 8) {
        let word = u64::from_le_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]]);
        let n = leading_digits(word);
        if n < 8 {
            if !VALUE {
                return at + n;
            }
            break;
        }
        if VALUE {
            *value = value.wrapping_mul(100_000_000).wrapping_add(eight_digits(word));
        }
        at += 8;
    }
    // fewer than eight: a short run is cheaper a digit at a time than through the word
    while let Some(&d @ b'0'..=b'9') = bytes.get(at) {
        if VALUE {
            *value = value.wrapping_mul(10).wrapping_add(u64::from(d - b'0'));
        }
        at += 1;
    }
    at
}

/// How many of `word`'s bytes, from its lowest, are ASCII digits: 0 to 8.
#[inline(always)]
fn leading_digits(word: u64) -> usize {
    // a digit's high nibble is 3, and stays 3 when 6 is added; a carry into the next byte comes only
    // from a byte that is not a digit, so the count up to the first such byte is exact
    const HIGH_NIBBLES: u64 = 0xF0F0_F0F0_F0F0_F0F0;
    let off = ((word & HIGH_NIBBLES) ^ ZEROS) | ((word.wrapping_add(0x0606_0606_0606_0606) & HIGH_NIBBLES) ^ ZEROS);
    (off.trailing_zeros() / 8) as usize
}

/// Eight ASCII digits, the first in the lowest byte, as their value: three multiplications, after
/// fast_float and simdjson.
#[inline(always)]
fn eight_digits(word: u64) -> u64 {
    const MASK: u64 = 0x0000_00FF_0000_00FF;
    const MUL1: u64 = 100 + (1_000_000 << 32);
    const MUL2: u64 = 1 + (10_000 << 32);
    let v = word - ZEROS;
    // each pair of digits as one value below 100, in the even bytes
    let v = v * 10 + (v >> 8);
    (v & MASK)
        .wrapping_mul(MUL1)
        .wrapping_add(((v >> 16) & MASK).wrapping_mul(MUL2))
        >> 32
}

#[cfg(test)]
mod tests {
    use super::{eight_digits, leading_digits};

    fn word(bytes: [u8; 8]) -> u64 {
        u64::from_le_bytes(bytes)
    }

    /// Bytes next to the digits, and those whose add carries into the next byte.
    const OTHERS: [u8; 12] = [b'/', b':', b'.', b'e', b'-', b' ', b'"', 0x00, 0x2f, 0x3a, 0xfa, 0xff];

    #[test]
    fn leading_digits_counts_to_the_first_byte_that_is_not_one() {
        let mut rng = 0x853C_49E6_748F_EA9Bu64;
        for n in 0..=8 {
            for &other in &OTHERS {
                for _ in 0..200 {
                    let mut b = [0u8; 8];
                    for (i, byte) in b.iter_mut().enumerate() {
                        rng ^= rng << 13;
                        rng ^= rng >> 7;
                        rng ^= rng << 17;
                        *byte = if i < n {
                            b'0' + (rng % 10) as u8
                        } else if i == n {
                            other
                        } else {
                            // anything after the first non-digit, digits included
                            (rng >> 8) as u8
                        };
                    }
                    assert_eq!(leading_digits(word(b)), n, "{b:?}");
                }
            }
        }
    }

    #[test]
    fn eight_digits_read_in_a_word_have_their_value() {
        let mut rng = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..100_000 {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let mut b = [0u8; 8];
            let mut value = 0u64;
            for (i, byte) in b.iter_mut().enumerate() {
                let d = ((rng >> (i * 4)) % 10) as u8;
                *byte = b'0' + d;
                value = value * 10 + u64::from(d);
            }
            assert_eq!(eight_digits(word(b)), value, "{b:?}");
        }
        assert_eq!(eight_digits(word(*b"99999999")), 99_999_999);
        assert_eq!(eight_digits(word(*b"00000000")), 0);
    }
}
