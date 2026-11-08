//! JSONTestSuite's parsing cases (tests/JSONTestSuite, see NOTICE): every `y_` file is accepted,
//! every `n_` file refused, and each `i_` file, which RFC 8259 leaves to the implementation, gets
//! the choice pinned below.

use std::path::Path;

/// The `i_` files the cursor accepts: numbers past a double's range, still in JSON's grammar, whose
/// text it gives as written. Every other `i_` file is refused: bytes that are not UTF-8, half of a
/// surrogate pair, a byte-order mark, or 500 nested arrays, past the skip's depth limit of 128.
const ACCEPTED: [&str; 10] = [
    "i_number_double_huge_neg_exp.json",
    "i_number_huge_exp.json",
    "i_number_neg_int_huge_exp.json",
    "i_number_pos_double_huge_exp.json",
    "i_number_real_neg_overflow.json",
    "i_number_real_pos_overflow.json",
    "i_number_real_underflow.json",
    "i_number_too_big_neg_int.json",
    "i_number_too_big_pos_int.json",
    "i_number_very_big_negative_int.json",
];

#[test]
fn every_case_of_json_test_suite_gets_its_answer() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/JSONTestSuite/test_parsing");
    let mut wrong = Vec::new();
    let mut counts = [0usize; 3];
    for entry in std::fs::read_dir(&dir).expect("the vendored corpus") {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        let accepted = json_cursor::validate(&std::fs::read(&path).unwrap()).is_ok();
        let want = match &name[..2] {
            "y_" => {
                counts[0] += 1;
                true
            }
            "n_" => {
                counts[1] += 1;
                false
            }
            "i_" => {
                counts[2] += 1;
                ACCEPTED.contains(&name.as_str())
            }
            _ => continue,
        };
        if accepted != want {
            wrong.push(format!("{name}: {}", if accepted { "accepted" } else { "refused" }));
        }
    }
    assert_eq!(counts, [95, 188, 35], "the whole corpus is there");
    wrong.sort();
    assert!(
        wrong.is_empty(),
        "{} cases answered otherwise:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
}
