use rikka_file_edit::Text;

fn text(bytes: &str) -> Text {
    Text::parse(bytes.as_bytes()).expect("it is text")
}

fn lines(bytes: &str) -> Vec<String> {
    text(bytes).lines().map(str::to_owned).collect()
}

#[test]
fn trailing_newline_terminates_last_line() {
    assert!(lines("").is_empty());
    assert_eq!(lines("foo"), ["foo"]);
    assert_eq!(lines("foo\n"), ["foo"]);
    assert_eq!(lines("\n"), [""]);
    assert_eq!(lines("a\n\n"), ["a", ""]);
    assert_eq!(lines("a\r\nb\r\n"), ["a", "b"]);

    let three = text("one\ntwo\nthree");
    assert_eq!(three.line_count(), 3);
    assert_eq!(three.line(3), Some("three"));
    assert_eq!(three.line(0), None);
    assert_eq!(
        text(
            "
"
        )
        .line_count(),
        1
    );
}

#[test]
fn rewrite_keeps_bom_eol_and_missing_newline() {
    assert_eq!(text("one\r\ntwo\r\n").render(), b"one\r\ntwo\r\n");
    assert_eq!(text("\u{feff}one\n").line(1), Some("one"));
    assert_eq!(
        text("one\ntwo").with_content("three\nfour\n").render(),
        b"three\nfour"
    );
    assert_eq!(
        text("\u{feff}one\r\n").with_content("two\nthree").render(),
        "\u{feff}two\r\nthree\r\n".as_bytes()
    );
    assert_eq!(
        text("one\r\n").with_content("a\r\nb\r\n").render(),
        b"a\r\nb\r\n"
    );
    assert_eq!(
        text("\u{feff}one\r\ntwo\r\n")
            .with_exact_content("one\ntwo")
            .render(),
        "\u{feff}one\r\ntwo".as_bytes()
    );
    assert_eq!(text("\u{feff}one\n").with_exact_content("").render(), b"");
    assert_eq!(Text::from_content("a\nb").render(), b"a\nb\n");
}

#[test]
fn rejects_binary_and_invalid_utf8() {
    assert!(Text::parse(&[0x00, 0x01]).is_err());
    assert!(Text::parse(&[0xff, 0xfe, 0x41]).is_err());
}
