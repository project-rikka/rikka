use browser_reader::{Block, Direction, Error, Inline, Limit, parse, readable};
use serde_json::{Value, json};

fn paragraph(text: &str) -> Value {
    json!({ "type": "paragraph", "content": [{ "type": "text", "text": text }] })
}

fn answer(blocks: Value) -> Value {
    json!({ "readable": true, "title": "Title", "blocks": blocks })
}

#[test]
fn parses_full_article() {
    let article = parse(&json!({
        "readable": true,
        "title": "  The   title\n ",
        "byline": "By Someone",
        "site": "Example News",
        "published": "2024-05-06T07:00:00+09:00",
        "lang": "ja-JP",
        "dir": "rtl",
        "image": { "src": "https://example.com/lead.jpg", "alt": "Lead", "width": 1200, "height": 630 },
        "truncated": true,
        "blocks": [paragraph("Hello world")]
    }))
    .expect("parse valid answer");
    assert_eq!(article.title(), "The title");
    assert_eq!(article.byline(), Some("By Someone"));
    assert_eq!(article.site(), Some("Example News"));
    assert_eq!(article.language(), Some("ja-JP"));
    assert_eq!(article.direction(), Direction::RightToLeft);
    assert_eq!(article.published_date().map(|date| date.day), Some(6));
    assert!(article.truncated());
    let lead = article.lead().expect("lead image");
    assert_eq!(lead.source.as_str(), "https://example.com/lead.jpg");
    assert_eq!((lead.width, lead.height), (Some(1200), Some(630)));
    assert_eq!(article.blocks().len(), 1);
    assert_eq!(article.reading().words, 2);
}

#[test]
fn probe_reports_unreadable() {
    let refusal = json!({ "readable": false, "reason": "short" });
    assert!(matches!(parse(&refusal), Err(Error::NotReadable)));
    assert!(!readable(&refusal).expect("valid probe result"));
    assert!(readable(&json!({ "readable": true, "mass": 2000 })).expect("valid probe result"));
    assert!(matches!(readable(&json!(null)), Err(Error::Malformed(_))));
    assert!(matches!(
        readable(&json!({ "readable": "yes" })),
        Err(Error::Malformed(_))
    ));
}

#[test]
fn rejects_malformed_answers() {
    assert!(matches!(
        parse(&json!("text")),
        Err(Error::Malformed("answer"))
    ));
    assert!(matches!(
        parse(&json!({ "readable": true, "blocks": [] })),
        Err(Error::Malformed("title"))
    ));
    assert!(matches!(
        parse(&json!({ "readable": true, "title": 5, "blocks": [] })),
        Err(Error::Malformed("title"))
    ));
    assert!(matches!(
        parse(&answer(json!([{ "type": "script", "text": "x" }]))),
        Err(Error::Malformed("block type"))
    ));
    assert!(matches!(
        parse(&answer(
            json!([{ "type": "paragraph", "content": [{ "type": "html", "html": "<b>" }] }])
        )),
        Err(Error::Malformed("inline type"))
    ));
    assert!(matches!(
        parse(&answer(json!({ "type": "paragraph" }))),
        Err(Error::Malformed("blocks"))
    ));
}

#[test]
fn unsafe_links_keep_text() {
    let article = parse(&answer(json!([{
        "type": "paragraph",
        "content": [
            { "type": "text", "text": "safe", "href": "https://example.com/" },
            { "type": "text", "text": " evil", "href": "javascript:alert(1)" },
            { "type": "text", "text": " data", "href": "data:text/html,<script>alert(1)</script>" }
        ]
    }])))
    .expect("parse answer with unsafe links");
    let Block::Paragraph(content) = &article.blocks()[0] else {
        panic!("expected paragraph");
    };
    let links = content
        .iter()
        .map(|inline| match inline {
            Inline::Text(run) => run.link.as_ref().map(|link| link.as_str().to_owned()),
            Inline::Break => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(links, [Some("https://example.com/".to_owned()), None, None]);
}

#[test]
fn unsafe_images_keep_caption() {
    let article = parse(&answer(json!([
        {
            "type": "figure",
            "images": [{ "src": "javascript:alert(1)", "alt": "x" }],
            "caption": [{ "type": "text", "text": "A caption" }]
        },
        {
            "type": "figure",
            "images": [{ "src": "data:text/html;base64,PHNjcmlwdD4=" }],
            "caption": []
        },
        {
            "type": "figure",
            "images": [{ "src": "data:image/png;base64,iVBORw0KGgo=", "width": 0, "height": 10 }],
            "caption": []
        }
    ])))
    .expect("parse answer with unsafe images");
    assert_eq!(article.blocks().len(), 2);
    assert!(matches!(&article.blocks()[0], Block::Paragraph(_)));
    let Block::Figure(figure) = &article.blocks()[1] else {
        panic!("expected figure");
    };
    assert!(figure.images[0].source.is_data());
    assert_eq!(
        (figure.images[0].width, figure.images[0].height),
        (None, None)
    );
}

#[test]
fn rejects_deep_nesting() {
    let mut blocks = json!([paragraph("deep")]);
    for _ in 0..10 {
        blocks = json!([{ "type": "quote", "blocks": blocks }]);
    }
    assert!(matches!(
        parse(&answer(blocks)),
        Err(Error::TooLarge(Limit::Nesting))
    ));
}

#[test]
fn rejects_too_many_blocks() {
    let blocks = Value::Array((0..9_000).map(|_| json!({ "type": "rule" })).collect());
    assert!(matches!(
        parse(&answer(blocks)),
        Err(Error::TooLarge(Limit::Blocks))
    ));
}

#[test]
fn rejects_too_much_text() {
    let chunk = "x".repeat(1024 * 1024);
    let blocks = Value::Array((0..7).map(|_| paragraph(&chunk)).collect());
    assert!(matches!(
        parse(&answer(blocks)),
        Err(Error::TooLarge(Limit::Text))
    ));
}

#[test]
fn rejects_oversized_tables() {
    let cell = json!({ "header": false, "content": [{ "type": "text", "text": "c" }] });
    let wide = json!([{ "type": "table", "rows": [Value::Array(vec![cell.clone(); 100])] }]);
    assert!(matches!(
        parse(&answer(wide)),
        Err(Error::TooLarge(Limit::Cells))
    ));
    let long = json!([{ "type": "table", "rows": Value::Array(vec![json!([cell]); 1_500]) }]);
    assert!(matches!(
        parse(&answer(long)),
        Err(Error::TooLarge(Limit::Rows))
    ));
}

#[test]
fn strips_control_characters() {
    let article = parse(&answer(json!([paragraph("a\u{0}b\u{1b}c")]))).expect("parse answer");
    let Block::Paragraph(content) = &article.blocks()[0] else {
        panic!("expected paragraph");
    };
    let Inline::Text(run) = &content[0] else {
        panic!("expected text run");
    };
    assert_eq!(run.text, "a b c");
}
