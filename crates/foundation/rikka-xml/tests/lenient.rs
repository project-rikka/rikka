use rikka_xml::{MAX_DEPTH, parse_lenient};

fn outline(source: &str) -> Option<String> {
    let root = parse_lenient(source)?;
    let mut outline = String::new();
    for element in root.descendants() {
        outline.push_str(element.name());
        if let Some(value) = element.attribute("v") {
            outline.push('=');
            outline.push_str(value);
        }
        let text = element.text();
        if !text.is_empty() {
            outline.push('"');
            outline.push_str(&text);
            outline.push('"');
        }
        outline.push(' ');
    }
    Some(outline.trim_end().to_owned())
}

#[test]
fn recovers_from_broken_markup() {
    let cases = [
        ("<a><b>one</b><c>two</c></a>", Some("a b\"one\" c\"two\"")),
        ("<a><b>one<c>two</a>", Some("a b\"one\" c\"two\"")),
        ("<a><b>one", Some("a b\"one\"")),
        ("<a></x><b/></a>", Some("a b")),
        ("<a v=1 w><b v='2'/></a>", Some("a=1 b=2")),
        ("<a>x &bogus; y &amp; z</a>", Some("a\"x &bogus; y & z\"")),
        ("<a>1 < 2</a>", Some("a\"1 < 2\"")),
        ("<a><b v=\"open></b><c/></a>", Some("a c")),
        ("lead <a/> trail <b/>", Some("a")),
        ("<p:a><p:b/></p:a>", Some("a b")),
        ("<a><!-- never closed", Some("a")),
        ("this is not xml", None),
        ("", None),
    ];
    for (source, expected) in cases {
        assert_eq!(outline(source).as_deref(), expected, "{source:?}");
    }
}

#[test]
fn gives_up_past_the_nesting_limit() {
    assert!(parse_lenient(&"<a>".repeat(MAX_DEPTH)).is_some());
    assert!(parse_lenient(&"<a>".repeat(MAX_DEPTH + 1)).is_none());
}
