//! What a walk adds up: the values seen, and the bytes of every key and string, decoded. Every
//! parser must reach the same sum on every message, so each one does the whole job.

use std::fmt;
use std::hint::black_box;

use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Sum {
    values: usize,
    text: usize,
}

impl std::ops::AddAssign for Sum {
    fn add_assign(&mut self, o: Sum) {
        self.values += o.values;
        self.text += o.text;
    }
}

pub(crate) fn one(text: usize) -> Sum {
    Sum { values: 1, text }
}

pub(crate) fn cursor_walk(
    c: &mut json_cursor::Cursor,
    keys: &mut String,
    text: &mut String,
) -> Result<Sum, json_cursor::Error> {
    use json_cursor::Value;
    let v = c.value(text)?;
    Ok(match v {
        Value::Object => {
            let mut s = one(0);
            let mut key = c.first_key(keys)?.map(str::len);
            while let Some(k) = key {
                s.text += k;
                s += cursor_walk(c, keys, text)?;
                key = c.next_key(keys)?.map(str::len);
            }
            s
        }
        Value::Array => {
            let mut s = one(0);
            let mut more = c.first_item()?;
            while more {
                s += cursor_walk(c, keys, text)?;
                more = c.next_item()?;
            }
            s
        }
        Value::String(t) => one(t.len()),
        Value::Number(n) => {
            // the value json-cursor read with the digits: an integer when written as one, else a double
            black_box(match n.is_integer() {
                true => n.as_i64().map_or_else(|| n.as_f64(), |i| i as f64),
                false => n.as_f64(),
            });
            one(0)
        }
        Value::Bool(b) => {
            black_box(b);
            one(0)
        }
        Value::Null => one(0),
    })
}

pub(crate) fn jiter_walk(j: &mut jiter::Jiter, peek: jiter::Peek) -> jiter::JiterResult<Sum> {
    use jiter::Peek;
    Ok(match peek {
        Peek::Object => {
            let mut s = one(0);
            let mut key = j.known_object()?.map(str::len);
            while let Some(k) = key {
                s.text += k;
                let p = j.peek()?;
                s += jiter_walk(j, p)?;
                key = j.next_key()?.map(str::len);
            }
            s
        }
        Peek::Array => {
            let mut s = one(0);
            let mut item = j.known_array()?;
            while let Some(p) = item {
                s += jiter_walk(j, p)?;
                item = j.array_step()?;
            }
            s
        }
        Peek::String => one(j.known_str()?.len()),
        Peek::True | Peek::False => {
            black_box(j.known_bool(peek)?);
            one(0)
        }
        Peek::Null => j.known_null().map(|()| one(0))?,
        p => {
            // jiter's own number parse, as its users read numbers
            black_box(j.known_number(p)?);
            one(0)
        }
    })
}

/// A serde value that is walked and added up, never built.
pub(crate) struct Walked(pub(crate) Sum);

impl<'de> Deserialize<'de> for Walked {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(Walk).map(Walked)
    }
}

struct Walk;

impl<'de> Visitor<'de> for Walk {
    type Value = Sum;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any value")
    }
    fn visit_bool<E>(self, v: bool) -> Result<Sum, E> {
        black_box(v);
        Ok(one(0))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Sum, E> {
        black_box(v);
        Ok(one(0))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Sum, E> {
        black_box(v);
        Ok(one(0))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Sum, E> {
        black_box(v);
        Ok(one(0))
    }
    fn visit_str<E>(self, v: &str) -> Result<Sum, E> {
        Ok(one(v.len()))
    }
    fn visit_unit<E>(self) -> Result<Sum, E> {
        Ok(one(0))
    }
    fn visit_none<E>(self) -> Result<Sum, E> {
        Ok(one(0))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Sum, A::Error> {
        let mut s = one(0);
        while let Some(Walked(x)) = seq.next_element()? {
            s += x;
        }
        Ok(s)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Sum, A::Error> {
        let mut s = one(0);
        while let Some(KeyLen(k)) = map.next_key()? {
            s.text += k;
            let Walked(x) = map.next_value()?;
            s += x;
        }
        Ok(s)
    }
}

/// A key's length, with no copy.
struct KeyLen(usize);

impl<'de> Deserialize<'de> for KeyLen {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl Visitor<'_> for V {
            type Value = usize;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a key")
            }
            fn visit_str<E>(self, v: &str) -> Result<usize, E> {
                Ok(v.len())
            }
        }
        d.deserialize_str(V).map(KeyLen)
    }
}

/// The text a serde_json `Value` holds, added up the same way.
pub(crate) fn value_sum(v: &serde_json::Value) -> Sum {
    use serde_json::Value;
    match v {
        Value::String(s) => one(s.len()),
        Value::Array(a) => {
            let mut s = one(0);
            for x in a {
                s += value_sum(x);
            }
            s
        }
        Value::Object(o) => {
            let mut s = one(0);
            for (k, x) in o {
                s.text += k.len();
                s += value_sum(x);
            }
            s
        }
        _ => one(0),
    }
}
