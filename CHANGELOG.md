# Changelog

All notable changes to json-cursor. We use [semantic versioning](https://semver.org), and each
release is tagged `vX.Y.Z`.

## Unreleased

The first release.

- 🧭 **A JSON pull cursor.** Read keys, strings, numbers, booleans and `null` one at a time, from
  a `&str` or from bytes. `value` reads whatever comes next, `skip` passes over anything, and
  `finish` checks the end. A string without escapes is borrowed from the input; one with escapes
  is decoded into a buffer you reuse.
- 🔢 **Exact numbers.** A `Number` keeps its text as written. `as_i64` gives its exact whole value
  however it is written, such as `7.0` or `1e3`, and `as_f64` the nearest double.
- 🛡️ **Strict.** Every byte is checked against RFC 8259, in skipped values too. Skipping keeps its
  own stack, with a depth limit of 128 by default.
- ✅ **`validate`** checks a whole document in one call.
- ⚡ **Fast.** In our benchmarks it reads every value faster than serde_json, sonic-rs, simd-json
  and jiter, on bids, events and GeoJSON.
- 🧪 **Tested** against JSONTestSuite, against serde_json on two million random documents, against
  Rust's own float parser, and by breaking 32 of its rules one at a time.
- No dependencies, no `unsafe` code, `no_std` with `alloc`, and Rust 1.85 or later.
