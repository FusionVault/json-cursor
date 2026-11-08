//! Time per message: a thread's CPU time, the median of 15 rounds, with the parsers taken in turn.

use std::hint::black_box;

use crate::messages::SHAPES;
use crate::parsers::{run, Bufs, Job, PARSERS};

fn thread_cpu_ns() -> u128 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: a valid out-pointer for the call
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as u128 * 1_000_000_000 + ts.tv_nsec as u128
}

/// Every shape, or only those named.
pub(crate) fn speed(only: &[String]) {
    println!("shape,bytes_per_message,job,parser,ns_per_message,min,max");
    for (shape, make, n) in SHAPES {
        if !only.is_empty() && !only.iter().any(|o| o == shape) {
            continue;
        }
        let msgs: Vec<Vec<u8>> = (0..n).map(|i| make(i).into_bytes()).collect();
        let bytes = msgs.iter().map(Vec::len).sum::<usize>() / n;
        for (job, job_name) in [(Job::Read, "read every value"), (Job::Validate, "validate")] {
            let parsers: Vec<&str> = PARSERS
                .iter()
                .copied()
                .filter(|p| matches!(job, Job::Read) || *p != "serde_json Value")
                .collect();
            // every parser reads the same sum from every message, or the race is not fair
            let mut want = Bufs::default();
            for p in &parsers {
                let mut b = Bufs::default();
                for m in &msgs {
                    assert_eq!(
                        run(p, job, m, &mut b),
                        run("json-cursor", job, m, &mut want),
                        "{p} reads {shape} otherwise"
                    );
                }
            }
            let mut rounds: Vec<Vec<f64>> = vec![Vec::new(); parsers.len()];
            let mut bufs: Vec<Bufs> = parsers.iter().map(|_| Bufs::default()).collect();
            for r in 0..15 {
                for k in 0..parsers.len() {
                    // each round starts from the next parser
                    let i = (k + r) % parsers.len();
                    let start = thread_cpu_ns();
                    for m in &msgs {
                        black_box(run(parsers[i], job, black_box(m), &mut bufs[i]));
                    }
                    rounds[i].push((thread_cpu_ns() - start) as f64 / n as f64);
                }
            }
            for (p, mut v) in parsers.iter().zip(rounds) {
                v.sort_by(f64::total_cmp);
                println!("{shape},{bytes},{job_name},{p},{:.1},{:.1},{:.1}", v[7], v[0], v[14]);
            }
        }
    }
}
