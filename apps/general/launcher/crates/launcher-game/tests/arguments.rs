use launcher_game::{Error, split_arguments};

#[test]
fn arguments_split_like_a_shell() {
    let cases: [(&str, &[&str]); 6] = [
        ("", &[]),
        ("  -XX:+UseG1GC   -Dx=1 ", &["-XX:+UseG1GC", "-Dx=1"]),
        (r#"-Dname="two words""#, &["-Dname=two words"]),
        ("-Da='single \"kept\"'", &["-Da=single \"kept\""]),
        (r#""" -Dy"#, &["", "-Dy"]),
        (r#""a\"b""#, &["a\"b"]),
    ];
    for (text, expected) in cases {
        assert_eq!(split_arguments(text).expect("splits"), expected, "{text}");
    }
    assert!(matches!(split_arguments("\"open"), Err(Error::Quote)));
}
