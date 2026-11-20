use rikka_xml::{Error, MAX_DEPTH, Markup, Position, parse};

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE library [ <!ELEMENT library ANY> ]>
<!-- shelves -->
<library name="Tom &amp; Jerry&#x2019;s">
  <shelf id='1'>
    <book>First</book>
    <book><![CDATA[<Second>]]> &lt;edition&gt;</book>
    <empty/>
  </shelf>
  <shelf id="2"/>
</library>
<!-- end -->
"#;

#[test]
fn reads_elements_attributes_and_text() {
    let library = parse(DOCUMENT).expect("well-formed");
    assert_eq!(library.name(), "library");
    assert_eq!(library.attribute("name"), Some("Tom & Jerry\u{2019}s"));
    assert_eq!(library.attribute("missing"), None);
    let shelves: Vec<_> = library.children_named("shelf").collect();
    assert_eq!(
        shelves
            .iter()
            .map(|shelf| shelf.attribute("id"))
            .collect::<Vec<_>>(),
        [Some("1"), Some("2")]
    );
    let books: Vec<String> = shelves[0]
        .children_named("book")
        .map(|book| book.text().into_owned())
        .collect();
    assert_eq!(books, ["First", "<Second> <edition>"]);
    assert_eq!(library.text(), "");
    let names: Vec<&str> = library
        .descendants()
        .map(|element| element.name())
        .collect();
    assert_eq!(
        names,
        ["library", "shelf", "book", "book", "empty", "shelf"]
    );
}

#[test]
fn ranges_cover_the_source_of_each_element() {
    let library = parse(DOCUMENT).expect("well-formed");
    let sources: Vec<&str> = library
        .descendants()
        .filter(|element| matches!(element.name(), "book" | "empty"))
        .map(|element| &DOCUMENT[element.range()])
        .collect();
    assert_eq!(
        sources,
        [
            "<book>First</book>",
            "<book><![CDATA[<Second>]]> &lt;edition&gt;</book>",
            "<empty/>",
        ]
    );
    let whole = &DOCUMENT[library.range()];
    assert!(whole.starts_with("<library name=") && whole.ends_with("</library>"));
}

#[test]
fn resolves_namespaces() {
    let root = parse(
        r#"<root xmlns="urn:default" xmlns:a="urn:a" xml:lang="en" a:kind="x" kind="y">
             <a:leaf/>
             <plain xmlns=""/>
           </root>"#,
    )
    .expect("well-formed");
    assert_eq!(root.namespace(), Some("urn:default"));
    assert_eq!(
        root.namespaces().collect::<Vec<_>>(),
        [(None, "urn:default"), (Some("a"), "urn:a")]
    );
    assert_eq!(root.attribute("kind"), Some("y"));
    assert_eq!(root.attribute_in("urn:a", "kind"), Some("x"));
    assert_eq!(
        root.attribute_in("http://www.w3.org/XML/1998/namespace", "lang"),
        Some("en")
    );
    let children: Vec<(&str, Option<&str>)> = root
        .children()
        .map(|child| (child.name(), child.namespace()))
        .collect();
    assert_eq!(children, [("leaf", Some("urn:a")), ("plain", None)]);
}

#[test]
fn rejects_malformed_documents() {
    let at = |line, column| Position { line, column };
    let cases = [
        ("", Error::NoRoot),
        ("just text", Error::OutsideRoot { at: at(1, 1) }),
        ("<a/><b/>", Error::OutsideRoot { at: at(1, 5) }),
        (
            "<a>\n  <b>\n</a>",
            Error::MismatchedEndTag {
                open: "b".into(),
                found: "a".into(),
                at: at(3, 1),
            },
        ),
        (
            "<a></b></a>",
            Error::UnexpectedEndTag {
                name: "b".into(),
                at: at(1, 4),
            },
        ),
        (
            "<a><b>",
            Error::UnclosedElement {
                name: "b".into(),
                at: at(1, 4),
            },
        ),
        (
            "<a><b",
            Error::Unterminated {
                markup: Markup::Tag,
                at: at(1, 4),
            },
        ),
        (
            "<a><!-- open</a>",
            Error::Unterminated {
                markup: Markup::Comment,
                at: at(1, 4),
            },
        ),
        ("<a b=1/>", Error::MalformedTag { at: at(1, 4) }),
        ("<a b/>", Error::MalformedTag { at: at(1, 4) }),
        ("<a b=\"1/>", Error::MalformedTag { at: at(1, 4) }),
        (
            "<a b=\"1\" b=\"2\"/>",
            Error::DuplicateAttribute {
                name: "b".into(),
                at: at(1, 1),
            },
        ),
        ("<a>&nbsp;</a>", Error::InvalidReference { at: at(1, 4) }),
        (
            "<a b=\"x &bogus; y\"/>",
            Error::InvalidReference { at: at(1, 9) },
        ),
        (
            "<p:a/>",
            Error::UndeclaredPrefix {
                prefix: "p".into(),
                at: at(1, 1),
            },
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(parse(source), Err(expected), "{source:?}");
    }
}

#[test]
fn nesting_is_limited() {
    let nested = |depth: usize| format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
    assert!(parse(&nested(MAX_DEPTH)).is_ok());
    assert!(matches!(
        parse(&nested(MAX_DEPTH + 1)),
        Err(Error::TooDeep { .. })
    ));
}
