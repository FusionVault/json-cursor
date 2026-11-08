//! Where one parser's time goes, for tuning: not part of the published results.

use std::hint::black_box;

use crate::messages::SHAPES;
use crate::parsers::{run, Bufs, Job};

/// Where one parser's time goes on one shape, sampled 1,999 times a second: the leaf functions
/// (inlined ones too, from the line tables), heaviest first.
pub(crate) fn profile(shape: &str, parser: &str, job: Job) {
    let (_, make, n) = SHAPES.iter().find(|s| s.0 == shape).copied().expect("a shape");
    let msgs: Vec<Vec<u8>> = (0..n).map(|i| make(i).into_bytes()).collect();
    let mut b = Bufs::default();
    let guard = pprof::ProfilerGuardBuilder::default().frequency(1999).build().unwrap();
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < 6 {
        for m in &msgs {
            black_box(run(parser, job, black_box(m), &mut b));
        }
    }
    let report = guard.report().build().unwrap();
    let mut leaf: std::collections::HashMap<String, isize> = Default::default();
    let mut total = 0;
    for (frames, count) in &report.data {
        total += count;
        // the innermost frame's source line, inlined code included
        let name = frames
            .frames
            .first()
            .and_then(|f| f.first())
            .map(|s| {
                let file = s.filename().to_string();
                let file = file.rsplit("/src/").next().unwrap_or(&file).to_string();
                format!("{file}:{} {}", s.lineno.unwrap_or(0), s.name())
            })
            .unwrap_or_else(|| "?".into());
        *leaf.entry(name).or_default() += count;
    }
    let mut rows: Vec<_> = leaf.into_iter().collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.1));
    println!("{parser} on {shape}: {total} samples");
    for (name, count) in rows.iter().take(25) {
        println!("{:5.1}%  {name}", *count as f64 * 100.0 / total as f64);
    }
}
