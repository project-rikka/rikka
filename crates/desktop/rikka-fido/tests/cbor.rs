use rikka_fido::cbor::{Error, Value};

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).expect("hex"))
        .collect()
}

#[test]
fn rfc_8949_examples_round_trip() {
    let examples: Vec<(&str, Value)> = vec![
        ("00", Value::Unsigned(0)),
        ("17", Value::Unsigned(23)),
        ("1818", Value::Unsigned(24)),
        ("1903e8", Value::Unsigned(1000)),
        ("1a000f4240", Value::Unsigned(1_000_000)),
        ("1b000000e8d4a51000", Value::Unsigned(1_000_000_000_000)),
        ("1bffffffffffffffff", Value::Unsigned(u64::MAX)),
        ("20", Value::from(-1)),
        ("3903e7", Value::from(-1000)),
        ("3bffffffffffffffff", Value::Negative(u64::MAX)),
        ("40", Value::Bytes(Vec::new())),
        ("4401020304", Value::bytes([1, 2, 3, 4])),
        ("60", Value::from("")),
        ("6161", Value::from("a")),
        ("6449455446", Value::from("IETF")),
        ("62225c", Value::from("\"\\")),
        ("62c3bc", Value::from("ü")),
        ("f4", Value::Bool(false)),
        ("f5", Value::Bool(true)),
        ("f6", Value::Null),
        ("f7", Value::Undefined),
        ("80", Value::Array(Vec::new())),
        ("83010203", Value::array([1, 2, 3])),
        (
            "8301820203820405",
            Value::array([Value::from(1), Value::array([2, 3]), Value::array([4, 5])]),
        ),
        ("a0", Value::Map(Vec::new())),
        ("a201020304", Value::map([(1, 2), (3, 4)])),
        (
            "a26161016162820203",
            Value::map([
                (Value::from("a"), Value::from(1)),
                (Value::from("b"), Value::array([2, 3])),
            ]),
        ),
        (
            "c074323031332d30332d32315432303a30343a30305a",
            Value::Tag(0, Box::new(Value::from("2013-03-21T20:04:00Z"))),
        ),
        ("fb3ff199999999999a", Value::Float(1.1)),
        ("fa47c35000", Value::Float(100_000.0)),
    ];

    for (encoded, value) in examples {
        assert_eq!(Value::decode(&hex(encoded)), Ok(value.clone()), "{encoded}");
        assert_eq!(value.encode(), hex(encoded), "{encoded}");
    }

    assert_eq!(
        Value::from(-1000).integer(),
        Some(-1000),
        "negative integers keep their value"
    );
    assert_eq!(
        Value::Negative(u64::MAX).integer(),
        Some(-18_446_744_073_709_551_616)
    );
}

#[test]
fn half_precision_floats_decode() {
    for (encoded, expected) in [("f93c00", 1.0), ("f97bff", 65504.0), ("f9c400", -4.0)] {
        assert_eq!(Value::decode(&hex(encoded)), Ok(Value::Float(expected)));
    }
}

#[test]
fn maps_encode_in_ctap2_canonical_order() {
    let scrambled = Value::map([
        (Value::from("aa"), Value::Null),
        (Value::from(-1), Value::Null),
        (Value::from("b"), Value::Null),
        (Value::from(300), Value::Null),
        (Value::from(1), Value::Null),
    ]);

    assert_eq!(
        scrambled.encode(),
        hex("a501f619012cf620f66162f6626161f6"),
        "lower major types first, then shorter keys, then bytewise"
    );
}

#[test]
fn rejects_malformed_input() {
    let nested = format!("{}00", "81".repeat(40));
    let cases: Vec<(String, Error)> = vec![
        ("5f".into(), Error::Indefinite),
        ("1a0000".into(), Error::Truncated),
        ("5bffffffffffffffff".into(), Error::Truncated),
        ("0000".into(), Error::Trailing(1)),
        ("a201000100".into(), Error::DuplicateKey),
        ("62c328".into(), Error::Utf8),
        ("1c".into(), Error::Malformed(0x1c)),
        ("f818".into(), Error::Simple(24)),
        (nested, Error::TooDeep(32)),
    ];

    for (encoded, expected) in cases {
        assert_eq!(Value::decode(&hex(&encoded)), Err(expected), "{encoded}");
    }
}
