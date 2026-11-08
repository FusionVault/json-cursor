//! What the integration tests share. Each test file uses its own part of it.
#![allow(dead_code)]

/// How many random cases a test runs: `JSON_CURSOR_CASES`, or `default`.
pub fn cases(default: usize) -> usize {
    std::env::var("JSON_CURSOR_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// xorshift64: a random source that gives the same values on every run.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A number below `n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    pub fn pick<'t, T>(&mut self, items: &'t [T]) -> &'t T {
        &items[self.below(items.len())]
    }
}
