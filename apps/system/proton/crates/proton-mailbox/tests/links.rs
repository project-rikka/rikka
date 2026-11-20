use proton_mailbox::{Link, Plain};

#[test]
fn plain_text_links_are_found() {
    let cases = [
        (
            "see https://example.com/a?b=c.",
            vec!["https://example.com/a?b=c"],
        ),
        (
            "(at http://example.com/wiki/Rust_(language)) or HTTPS://EXAMPLE.ORG/x!",
            vec![
                "http://example.com/wiki/Rust_(language)",
                "HTTPS://EXAMPLE.ORG/x",
            ],
        ),
        (
            "詳細はhttps://www.pixiv.net/historyで確認",
            vec!["https://www.pixiv.net/history"],
        ),
        (
            "write <mailto:help@example.com>, not xhttps://no or https:// alone",
            vec!["mailto:help@example.com"],
        ),
    ];
    for (text, expected) in cases {
        let plain = Plain::new(text);
        let found: Vec<&str> = plain
            .links()
            .iter()
            .map(|range| &text[range.clone()])
            .collect();
        assert_eq!(found, expected, "{text}");
    }
}

#[test]
fn links_open_only_web_and_mail() {
    match Link::parse(" https://example.com/path?q=1 ") {
        Some(Link::Web(url)) => assert_eq!(url.as_str(), "https://example.com/path?q=1"),
        other => panic!("{other:?}"),
    }
    match Link::parse(
        "mailto:ana@proton.me,bo@proton.me?cc=cy@proton.me&Subject=Hi%20there&body=line%0Anext",
    ) {
        Some(Link::Compose(draft)) => {
            assert_eq!(draft.to, "ana@proton.me, bo@proton.me");
            assert_eq!(draft.cc, "cy@proton.me");
            assert_eq!(draft.subject, "Hi there");
            assert_eq!(draft.body, "line\nnext");
        }
        other => panic!("{other:?}"),
    }
    for refused in [
        "javascript:alert(1)",
        "/relative",
        "file:///etc/passwd",
        "cid:logo",
    ] {
        assert!(Link::parse(refused).is_none(), "{refused}");
    }
}
