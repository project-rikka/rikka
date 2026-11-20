use rikka_html::{Token, tokens};

#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Text(String),
    Open(String, Vec<(String, String)>, bool),
    Close(String),
}

fn seen(html: &str) -> Vec<Seen> {
    tokens(html)
        .map(|token| match token {
            Token::Text(text) => Seen::Text(text.into_owned()),
            Token::Open(tag) => Seen::Open(
                tag.name().to_owned(),
                tag.attributes()
                    .map(|(name, value)| (name.to_owned(), value.to_owned()))
                    .collect(),
                tag.is_self_closing(),
            ),
            Token::Close(name) => Seen::Close(name.into_owned()),
        })
        .collect()
}

fn text(text: &str) -> Seen {
    Seen::Text(text.to_owned())
}

fn open(name: &str, attributes: &[(&str, &str)]) -> Seen {
    Seen::Open(
        name.to_owned(),
        attributes
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        false,
    )
}

fn close(name: &str) -> Seen {
    Seen::Close(name.to_owned())
}

#[test]
fn tags_attributes_and_text_are_told_apart() {
    let cases = [
        (
            "<A HREF=\"/x?a=1&amp;b=2\" Class=big data-n=3 hidden>T&amp;C</A>",
            vec![
                open(
                    "a",
                    &[
                        ("href", "/x?a=1&b=2"),
                        ("class", "big"),
                        ("data-n", "3"),
                        ("hidden", ""),
                    ],
                ),
                text("T&C"),
                close("a"),
            ],
        ),
        (
            "1 < 2 <!-- <b> --><!DOCTYPE html><?pi?>3",
            vec![text("1 < 2 "), text("3")],
        ),
        (
            "<a i=7/>x<br />",
            vec![
                Seen::Open("a".to_owned(), vec![("i".to_owned(), "7".to_owned())], true),
                text("x"),
                Seen::Open("br".to_owned(), Vec::new(), true),
            ],
        ),
        ("a <b", vec![text("a <b")]),
    ];
    for (html, expected) in cases {
        assert_eq!(seen(html), expected, "{html}");
    }
}

#[test]
fn raw_text_elements_hide_their_markup() {
    let cases = [
        (
            "<script>if (a < b) { x = \"</div>&amp;\" }</SCRIPT>after",
            vec![
                open("script", &[]),
                text("if (a < b) { x = \"</div>&amp;\" }"),
                close("script"),
                text("after"),
            ],
        ),
        (
            "<title>A &amp; <b></title>",
            vec![open("title", &[]), text("A & <b>"), close("title")],
        ),
        (
            "<style>p > a {}",
            vec![open("style", &[]), text("p > a {}")],
        ),
    ];
    for (html, expected) in cases {
        assert_eq!(seen(html), expected, "{html}");
    }
}
