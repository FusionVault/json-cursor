//! RFC 8259, case by case: what the cursor accepts, and what it refuses, with which error.

use json_cursor::{validate, Cursor, ErrorKind, Kind};

/// A cursor over test bytes, which are UTF-8.
fn cursor(bytes: &[u8]) -> Cursor<'_> {
    Cursor::from_bytes(bytes).unwrap()
}

#[track_caller]
fn accepts(cases: &[&[u8]]) {
    for case in cases {
        if let Err(e) = validate(case) {
            panic!("refused {:?}: {e}", String::from_utf8_lossy(case));
        }
    }
}

#[track_caller]
fn refuses(cases: &[(&[u8], ErrorKind)]) {
    for (case, kind) in cases {
        match validate(case) {
            Ok(()) => panic!("accepted {:?}", String::from_utf8_lossy(case)),
            Err(e) => assert_eq!(e.kind(), *kind, "{:?}: {e}", String::from_utf8_lossy(case)),
        }
    }
}

#[test]
fn every_kind_of_value_is_accepted() {
    accepts(&[
        b"null",
        b"true",
        b"false",
        b"0",
        b"-0",
        b"1.5",
        b"-1.5e-3",
        b"1E+2",
        b"123456789012345678901234567890",
        b"1e400",
        br#""""#,
        br#""a""#,
        b"[]",
        b"{}",
        b"[1,[2,[3]]]",
        br#"{"a":{"b":[]},"c":null}"#,
        b" \t\r\n[ 1 , 2 ] \n",
    ]);
}

#[test]
fn a_number_outside_the_grammar_is_refused() {
    refuses(&[
        (b"-", ErrorKind::Number),
        (b"--1", ErrorKind::Number),
        (b"-a", ErrorKind::Number),
        (b"1.", ErrorKind::Number),
        (b"1.e5", ErrorKind::Number),
        (b"-.5", ErrorKind::Number),
        (b"1e", ErrorKind::Number),
        (b"1e+", ErrorKind::Number),
        (b"-Infinity", ErrorKind::Number),
        (b"+1", ErrorKind::Expected("a value")),
        (b".5", ErrorKind::Expected("a value")),
        (b"NaN", ErrorKind::Expected("a value")),
        (b"Infinity", ErrorKind::Expected("a value")),
        // a number ends where its grammar does, and what follows is refused
        (b"01", ErrorKind::Trailing),
        (b"0x1", ErrorKind::Trailing),
        (b"[01]", ErrorKind::Expected("`,` or `]`")),
        (b"1-2", ErrorKind::Trailing),
    ]);
}

#[test]
fn a_string_holds_escaped_control_characters_and_utf8_only() {
    accepts(&[
        br#""\u00e9""#,
        br#""\ud83d\ude00""#,
        "\"é漢😀\"".as_bytes(),
        br#""\"\\\/\b\f\n\r\t""#,
        b"\"\x7f\"",
    ]);
    refuses(&[
        (b"\"\x01\"", ErrorKind::ControlCharacter),
        (b"\"a\nb\"", ErrorKind::ControlCharacter),
        (br#""\x""#, ErrorKind::Escape),
        (br#""\u12""#, ErrorKind::Escape),
        (br#""\u12"#, ErrorKind::End),
        (br#""\ud800""#, ErrorKind::Surrogate),
        (br#""\udc00""#, ErrorKind::Surrogate),
        (br#""\ud800A""#, ErrorKind::Surrogate),
        (br#""\ud800\u0041""#, ErrorKind::Surrogate),
        (b"\"\xff\"", ErrorKind::Utf8),
        (b"\"\xed\xa0\x80\"", ErrorKind::Utf8),
        (b"\"\xc0\xaf\"", ErrorKind::Utf8),
        (b"\"\xe2\x82\"", ErrorKind::Utf8),
        (b"\"a\\n\xff\"", ErrorKind::Utf8),
        (b"\"\xff\\n\"", ErrorKind::Utf8),
        (b"\"abc", ErrorKind::End),
        (b"\"\\", ErrorKind::End),
    ]);
}

#[test]
fn objects_and_arrays_need_their_commas_and_colons() {
    accepts(&[br#"{"a":1,"a":2}"#]);
    refuses(&[
        (b"[1,]", ErrorKind::Expected("a value")),
        (b"[,1]", ErrorKind::Expected("a value")),
        (b"[1 2]", ErrorKind::Expected("`,` or `]`")),
        (b"[1:2]", ErrorKind::Expected("`,` or `]`")),
        (br#"{"a":1,}"#, ErrorKind::Expected("a key")),
        (br#"{"a":1,,"b":2}"#, ErrorKind::Expected("a key")),
        (br#"{"a" 1}"#, ErrorKind::Expected("`:`")),
        (br#"{"a":1 "b":2}"#, ErrorKind::Expected("`,` or `}`")),
        (br#"{"a":1]"#, ErrorKind::Expected("`,` or `}`")),
        (b"[1}", ErrorKind::Expected("`,` or `]`")),
        (b"{1:2}", ErrorKind::Expected("a key or `}`")),
        (br#"{"a"}"#, ErrorKind::Expected("`:`")),
        (b"[", ErrorKind::End),
        (b"{", ErrorKind::End),
        (br#"{"a""#, ErrorKind::End),
        (b"[1", ErrorKind::End),
        (br#"{"a":1"#, ErrorKind::End),
        (b"nul", ErrorKind::End),
        (b"nulx", ErrorKind::Expected("`null`")),
        (b"tru", ErrorKind::End),
        (b"fals", ErrorKind::End),
        (b"trUe", ErrorKind::Expected("`true`")),
    ]);
}

#[test]
fn whitespace_is_space_tab_and_line_ends_only() {
    accepts(&[b" 1 ", b"\t\r\n1"]);
    refuses(&[
        ("\u{feff}1".as_bytes(), ErrorKind::Expected("a value")),
        (b"\x0c1", ErrorKind::Expected("a value")),
        (b"1\x00", ErrorKind::Trailing),
    ]);
}

#[test]
fn bytes_after_the_value_and_an_empty_input_are_refused() {
    refuses(&[
        (b"1 2", ErrorKind::Trailing),
        (b"{}x", ErrorKind::Trailing),
        (b"[]]", ErrorKind::Trailing),
        (b"", ErrorKind::End),
        (b"   ", ErrorKind::End),
    ]);
}

#[test]
fn skip_stops_at_its_depth_limit_without_using_the_call_stack() {
    let nested = |n: usize| [b"[".repeat(n), b"]".repeat(n)].concat();
    assert!(validate(&nested(128)).is_ok());
    assert_eq!(validate(&nested(129)).unwrap_err().kind(), ErrorKind::TooDeep);
    let deep = nested(1_000_000);
    let mut cursor = Cursor::from_bytes(&deep).unwrap().with_depth_limit(1_000_000);
    assert!(cursor.skip().is_ok(), "a million deep, on the skip's own stack");
    assert!(cursor.finish().is_ok());
    let objects = [br#"{"a":"#.repeat(10), b"1".to_vec(), b"}".repeat(10)].concat();
    let mut cursor = Cursor::from_bytes(&objects).unwrap().with_depth_limit(9);
    assert_eq!(cursor.skip().unwrap_err().kind(), ErrorKind::TooDeep);
}

#[test]
fn an_error_says_where() {
    let e = validate(b"[1,]").unwrap_err();
    assert_eq!(e.position(), 3);
    assert_eq!(e.to_string(), "expected a value at byte 3");
    assert_eq!(validate(b"\"a\xff\"").unwrap_err().position(), 2);
    assert_eq!(validate(br#""\u00zz""#).unwrap_err().position(), 5);
}

#[test]
fn a_number_is_its_exact_text() {
    let mut c = cursor(b"[12.50, -0, 1E+2, 123456789012345678901234567890, 1e400]");
    assert!(c.first_item().unwrap());
    for want in ["12.50", "-0", "1E+2", "123456789012345678901234567890", "1e400"] {
        assert_eq!(c.number().unwrap().text(), want);
        c.next_item().unwrap();
    }
}

#[test]
fn a_string_without_escapes_is_borrowed_and_one_with_them_is_decoded() {
    let input = br#"["plain", "a\"b\u00e9\ud83d\ude00"]"#;
    let mut c = cursor(input);
    let mut buf = String::new();
    assert!(c.first_item().unwrap());
    let plain = c.string(&mut buf).unwrap();
    assert_eq!(plain, "plain");
    assert!(
        input.as_ptr_range().contains(&plain.as_ptr()),
        "borrowed from the input"
    );
    assert!(c.next_item().unwrap());
    assert_eq!(c.string(&mut buf).unwrap(), "a\"bé😀");
    assert_eq!(buf, "a\"bé😀", "decoded into the buffer");
    assert!(!c.next_item().unwrap());
    c.finish().unwrap();
}

#[test]
fn keys_and_values_come_in_order() {
    let mut c = cursor(br#" {"a": 1, "b": {"c": [true, null]}, "d": "x"} "#);
    let (mut kb, mut vb) = (String::new(), String::new());
    let mut seen = Vec::new();
    let mut key = c.first_key(&mut kb).unwrap().map(str::to_string);
    while let Some(k) = key {
        let kind = c.peek().unwrap();
        seen.push(format!("{k}:{kind:?}"));
        match kind {
            Kind::Object => {
                let inner = c.first_key(&mut vb).unwrap().unwrap();
                assert_eq!(inner, "c");
                assert!(c.first_item().unwrap());
                assert!(c.bool().unwrap());
                assert!(c.next_item().unwrap());
                c.null().unwrap();
                assert!(!c.next_item().unwrap());
                assert_eq!(c.next_key(&mut vb).unwrap(), None);
            }
            Kind::Number => assert_eq!(c.number().unwrap().text(), "1"),
            Kind::String => assert_eq!(c.string(&mut vb).unwrap(), "x"),
            other => panic!("{other:?}"),
        }
        key = c.next_key(&mut kb).unwrap().map(str::to_string);
    }
    c.finish().unwrap();
    assert_eq!(seen, ["a:Number", "b:Object", "d:String"]);
}

#[test]
fn reading_another_kind_than_the_next_is_refused() {
    let mut buf = String::new();
    assert_eq!(
        cursor(b"1").string(&mut buf).unwrap_err().kind(),
        ErrorKind::Expected("a string")
    );
    assert_eq!(
        cursor(b"\"1\"").number().unwrap_err().kind(),
        ErrorKind::Expected("a number")
    );
    assert_eq!(
        cursor(b"null").bool().unwrap_err().kind(),
        ErrorKind::Expected("`true` or `false`")
    );
    assert_eq!(
        cursor(b"[]").first_key(&mut buf).unwrap_err().kind(),
        ErrorKind::Expected("`{`")
    );
    assert_eq!(
        cursor(b"{}").first_item().unwrap_err().kind(),
        ErrorKind::Expected("`[`")
    );
}
