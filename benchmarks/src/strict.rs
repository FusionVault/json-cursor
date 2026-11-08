//! What each parser accepts: 25 messages with RFC 8259's answer, and a million nested arrays.

use std::process::Command;

use crate::parsers::{run, Bufs, Job, PARSERS};

/// RFC 8259's answer: `Some(true)` must accept, `Some(false)` must refuse, `None` is the parser's call.
fn cases() -> Vec<(&'static str, Vec<u8>, Option<bool>)> {
    vec![
        ("an object", br#"{"a":[1,2.5,"x",true,null]}"#.to_vec(), Some(true)),
        ("-0 and 1E+2", b"[-0,1E+2]".to_vec(), Some(true)),
        ("1e400 past a double's range", b"[1e400]".to_vec(), Some(true)),
        (
            "a 30-digit integer",
            b"[123456789012345678901234567890]".to_vec(),
            Some(true),
        ),
        ("a repeated key", br#"{"a":1,"a":2}"#.to_vec(), Some(true)),
        ("a trailing comma in an array", b"[1,]".to_vec(), Some(false)),
        ("a trailing comma in an object", br#"{"a":1,}"#.to_vec(), Some(false)),
        ("two commas", br#"{"a":1,,"b":2}"#.to_vec(), Some(false)),
        ("a missing comma", br#"{"a":1 "b":2}"#.to_vec(), Some(false)),
        ("1-2", b"[1-2]".to_vec(), Some(false)),
        ("a leading zero", b"[01]".to_vec(), Some(false)),
        ("NaN", b"[NaN]".to_vec(), Some(false)),
        ("a lone minus", b"[-]".to_vec(), Some(false)),
        ("a raw control character", b"[\"a\x01b\"]".to_vec(), Some(false)),
        ("trailing bytes", br#"{"a":1}x"#.to_vec(), Some(false)),
        ("an empty input", Vec::new(), Some(false)),
        ("a bad escape", br#"["\x"]"#.to_vec(), Some(false)),
        ("bad UTF-8 in a string", b"[\"\xff\"]".to_vec(), Some(false)),
        ("bad UTF-8 in a key", b"{\"\xff\":1}".to_vec(), Some(false)),
        (
            "bad UTF-8 in a nested string",
            b"{\"a\":{\"b\":\"\xff\"}}".to_vec(),
            Some(false),
        ),
        ("an overlong UTF-8 sequence", b"[\"\xc0\xaf\"]".to_vec(), Some(false)),
        (
            "a surrogate encoded as UTF-8",
            b"[\"\xed\xa0\x80\"]".to_vec(),
            Some(false),
        ),
        ("a byte-order mark", b"\xef\xbb\xbf{}".to_vec(), None),
        (
            "a lone surrogate escape",
            [b"[\"".as_slice(), b"\\", b"ud800\"]"].concat(),
            None,
        ),
        ("300 nested arrays", [b"[".repeat(300), b"]".repeat(300)].concat(), None),
    ]
}

/// The job that tells whether a parser takes a message: `Value` builds one, the rest validate.
fn job_of(parser: &str) -> Job {
    if parser == "serde_json Value" {
        Job::Read
    } else {
        Job::Validate
    }
}

fn answer(accepted: bool) -> &'static str {
    if accepted {
        "accepts"
    } else {
        "refuses"
    }
}

/// Each case, with RFC 8259's answer and each parser's, as CSV.
pub(crate) fn strict() {
    println!("case,rfc_8259,{}", PARSERS.join(","));
    for (case, msg, want) in cases() {
        let rfc = match want {
            Some(true) => "accept",
            Some(false) => "refuse",
            None => "either",
        };
        let answers: Vec<&str> = PARSERS
            .iter()
            .map(|p| answer(run(p, job_of(p), &msg, &mut Bufs::default()).is_some()))
            .collect();
        println!("{case},{rfc},{}", answers.join(","));
    }
    // a stack overflow ends the process and cannot be caught, so each parser meets this one alone
    let me = std::env::current_exe().expect("this program's path");
    let answers: Vec<String> = PARSERS
        .iter()
        .map(|p| {
            let out = Command::new(&me).args(["deep", p]).output().expect("a child process");
            if out.status.success() {
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            } else {
                "crashes".to_string()
            }
        })
        .collect();
    println!("a million nested arrays under one key,either,{}", answers.join(","));
}

/// A million nested arrays under a key, met by one parser, in the child process `strict` starts.
pub(crate) fn deep(parser: &str) {
    let msg = [
        br#"{"x":"#.as_slice(),
        &b"[".repeat(1_000_000),
        &b"]".repeat(1_000_000),
        b",\"a\":1}",
    ]
    .concat();
    println!(
        "{}",
        answer(run(parser, job_of(parser), &msg, &mut Bufs::default()).is_some())
    );
}
