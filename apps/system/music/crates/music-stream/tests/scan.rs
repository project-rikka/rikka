use music_stream::Scan;

fn quoted_digits(text: &str, prefix: &str, count: usize) -> Vec<String> {
    Scan::new(text)
        .matches(prefix, |mut scan| {
            let digits = scan.take(count..=count, |c| c.is_ascii_digit())?;
            scan.expect("\"")?;
            Some(digits.to_owned())
        })
        .collect()
}

#[test]
fn matches_skip_candidates_that_do_not_parse() {
    let cases: &[(&str, &str, usize, &[&str])] = &[
        ("exact", r#"id:"123456789""#, 9, &["123456789"]),
        ("too long", r#"id:"1234567890""#, 9, &[]),
        ("too short", r#"id:"12345678""#, 9, &[]),
        (
            "second candidate",
            r#"id:"12x" id:"987654321" id:"111111111""#,
            9,
            &["987654321", "111111111"],
        ),
        ("unicode around", r#"é id:"42" ü"#, 2, &["42"]),
    ];
    for (name, text, count, expected) in cases {
        assert_eq!(quoted_digits(text, r#"id:""#, *count), *expected, "{name}");
    }
}

#[test]
fn take_is_greedy_within_bounds() {
    let mut scan = Scan::new("abcdef1");
    assert_eq!(scan.take(2..=4, |c| c.is_ascii_lowercase()), Some("abcd"));
    assert_eq!(scan.take(3..=9, |c| c.is_ascii_lowercase()), None);
    assert_eq!(scan.rest(), "ef1");
    scan.skip_while(|c| c.is_ascii_lowercase());
    scan.skip("1");
    assert_eq!(scan.rest(), "");
}
