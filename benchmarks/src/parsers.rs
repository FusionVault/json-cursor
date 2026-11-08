//! The parsers raced, each reading one message through its own natural API.

use serde::de::IgnoredAny;

use crate::walk::{cursor_walk, jiter_walk, one, value_sum, Sum, Walked};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Job {
    /// Every value, ready to use.
    Read,
    /// Whether the message is JSON.
    Validate,
}

pub(crate) const PARSERS: [&str; 6] = [
    "json-cursor",
    "jiter",
    "serde_json",
    "sonic-rs",
    "simd-json",
    "serde_json Value",
];

/// The buffers a parser reuses across messages.
#[derive(Default)]
pub(crate) struct Bufs {
    keys: String,
    text: String,
    copy: Vec<u8>,
    simd: Option<simd_json::Buffers>,
}

/// The sum one parser reads from one message, or `None` when it refuses it.
pub(crate) fn run(parser: &str, job: Job, msg: &[u8], b: &mut Bufs) -> Option<Sum> {
    match (parser, job) {
        ("json-cursor", Job::Read) => {
            let mut c = json_cursor::Cursor::from_bytes(msg).ok()?;
            let s = cursor_walk(&mut c, &mut b.keys, &mut b.text).ok()?;
            c.finish().ok()?;
            Some(s)
        }
        ("json-cursor", Job::Validate) => json_cursor::validate(msg).ok().map(|()| one(0)),
        ("jiter", Job::Read) => {
            let mut j = jiter::Jiter::new(msg);
            let p = j.peek().ok()?;
            let s = jiter_walk(&mut j, p).ok()?;
            j.finish().ok()?;
            Some(s)
        }
        ("jiter", Job::Validate) => {
            let mut j = jiter::Jiter::new(msg);
            j.next_skip().ok()?;
            j.finish().ok().map(|()| one(0))
        }
        ("serde_json", Job::Read) => serde_json::from_slice::<Walked>(msg).ok().map(|w| w.0),
        ("serde_json", Job::Validate) => serde_json::from_slice::<IgnoredAny>(msg).ok().map(|_| one(0)),
        ("sonic-rs", Job::Read) => sonic_rs::from_slice::<Walked>(msg).ok().map(|w| w.0),
        ("sonic-rs", Job::Validate) => sonic_rs::from_slice::<IgnoredAny>(msg).ok().map(|_| one(0)),
        ("simd-json", job) => {
            b.copy.clear();
            b.copy.extend_from_slice(msg);
            let bufs = b.simd.get_or_insert_with(|| simd_json::Buffers::new(4096));
            match job {
                Job::Read => simd_json::serde::from_slice_with_buffers::<Walked>(&mut b.copy, bufs)
                    .ok()
                    .map(|w| w.0),
                Job::Validate => simd_json::serde::from_slice_with_buffers::<IgnoredAny>(&mut b.copy, bufs)
                    .ok()
                    .map(|_| one(0)),
            }
        }
        ("serde_json Value", Job::Read) => serde_json::from_slice::<serde_json::Value>(msg)
            .ok()
            .map(|v| value_sum(&v)),
        ("serde_json Value", Job::Validate) => serde_json::from_slice::<serde_json::Value>(msg).ok().map(|_| one(0)),
        _ => None,
    }
}
