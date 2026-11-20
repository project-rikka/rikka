use browser_reader::{Article, Labels, Look, parse, render};
use serde_json::{Value, json};

fn article(fields: Value) -> Article {
    let mut answer = json!({
        "readable": true,
        "title": "Title",
        "blocks": [{ "type": "paragraph", "content": [{ "type": "text", "text": "Body" }] }]
    });
    for (key, value) in fields.as_object().expect("object fields") {
        answer[key] = value.clone();
    }
    parse(&answer).expect("parse valid answer")
}

fn html(article: &Article) -> String {
    render(article, &Look::default(), &Labels::default())
}

fn body(html: &str) -> &str {
    let start = html.find("<body>").expect("body element");
    &html[start..]
}

#[test]
fn document_is_standalone_with_csp() {
    let page = html(&article(json!({ "lang": "en", "dir": "ltr" })));
    assert!(page.starts_with("<!DOCTYPE html>\n<html lang=\"en\" dir=\"ltr\">"));
    assert!(page.contains(
        "<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src https: http: data:; style-src 'unsafe-inline'\">"
    ));
    let policy = page
        .find("Content-Security-Policy")
        .expect("content security policy");
    assert!(policy < page.find("<style>").expect("style element"));
    assert!(!page.contains("<script"));
    assert!(page.contains("<meta charset=\"utf-8\">"));
    assert!(page.ends_with("</html>\n"));
}

#[test]
fn escapes_hostile_text() {
    let hostile = "</title><script>alert(1)</script>\"'&";
    let page = html(&article(json!({
        "title": hostile,
        "byline": hostile,
        "site": hostile,
        "image": { "src": "https://example.com/a.jpg?x=\"><script>", "alt": hostile },
        "blocks": [
            { "type": "heading", "level": 2, "content": [{ "type": "text", "text": hostile }] },
            { "type": "paragraph", "content": [{ "type": "text", "text": hostile, "href": "https://example.com/\"onmouseover=\"x" }] },
            { "type": "code", "text": hostile, "language": "html" },
            { "type": "table", "caption": [{ "type": "text", "text": hostile }], "rows": [[{ "header": true, "content": [{ "type": "text", "text": hostile }] }]] },
            { "type": "list", "items": [[{ "type": "paragraph", "content": [{ "type": "text", "text": hostile, "ruby": hostile }] }]] }
        ]
    })));
    assert!(!page.contains("<script"));
    assert!(!page.contains("</title><"));
    assert!(!page.contains("\"onmouseover"));
    assert!(page.contains("a.jpg?x=&quot;&gt;&lt;script&gt;"));
    assert!(page.contains("&lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt;&quot;&#39;&amp;"));
}

#[test]
fn drops_dangerous_urls() {
    let page = html(&article(json!({
        "image": { "src": "javascript:alert(1)" },
        "blocks": [
            { "type": "paragraph", "content": [{ "type": "text", "text": "click", "href": "javascript:alert(1)" }] },
            { "type": "paragraph", "content": [{ "type": "text", "text": "file", "href": "file:///etc/passwd" }] },
            { "type": "figure", "images": [{ "src": "data:text/html,<script>alert(1)</script>" }, { "src": "https://example.com/ok.png" }], "caption": [] }
        ]
    })));
    let content = body(&page);
    assert!(!content.contains("javascript:"));
    assert!(!content.contains("file:"));
    assert!(!content.contains("data:text"));
    assert!(content.contains("<p>click</p>"));
    assert!(content.contains("<img src=\"https://example.com/ok.png\""));
}

#[test]
fn renders_every_block_kind() {
    let text = |text: &str| json!([{ "type": "text", "text": text }]);
    let page = html(&article(json!({
        "blocks": [
            { "type": "heading", "level": 1, "content": text("Top") },
            { "type": "heading", "level": 3, "content": text("Sub") },
            { "type": "paragraph", "content": text("Para") },
            { "type": "list", "ordered": true, "start": 3, "items": [
                [{ "type": "paragraph", "content": text("three") }],
                [{ "type": "paragraph", "content": text("four") }, { "type": "list", "items": [[{ "type": "paragraph", "content": text("inner") }]] }]
            ] },
            { "type": "quote", "blocks": [{ "type": "paragraph", "content": text("Quoted") }] },
            { "type": "code", "text": "let x = 1;\n  x < 2", "language": "rust" },
            { "type": "figure", "images": [{ "src": "https://example.com/a.jpg", "alt": "A", "width": 800, "height": 600 }], "caption": text("Cap") },
            { "type": "table", "caption": text("T"), "rows": [
                [{ "header": true, "content": text("h"), "colspan": 2 }],
                [{ "header": false, "content": text("1"), "rowspan": 2 }, { "header": false, "content": text("2") }]
            ] },
            { "type": "rule" }
        ]
    })));
    let content = body(&page);
    assert!(content.contains("<h2>Top</h2>"));
    assert!(content.contains("<h4>Sub</h4>"));
    assert!(content.contains("<p>Para</p>"));
    assert!(content.contains("<ol start=\"3\">\n<li>three</li>\n<li>\n<p>four</p>\n<ul>\n<li>inner</li>\n</ul>\n</li>\n</ol>"));
    assert!(content.contains("<blockquote>\n<p>Quoted</p>\n</blockquote>"));
    assert!(
        content.contains("<pre><code class=\"language-rust\">let x = 1;\n  x &lt; 2</code></pre>")
    );
    assert!(content.contains(
        "<figure>\n<img src=\"https://example.com/a.jpg\" alt=\"A\" width=\"800\" height=\"600\" loading=\"lazy\" decoding=\"async\">\n<figcaption>Cap</figcaption>\n</figure>"
    ));
    assert!(content.contains("<div class=\"table\"><table>\n<caption>T</caption>\n<tbody>\n<tr><th colspan=\"2\">h</th></tr>\n<tr><td rowspan=\"2\">1</td><td>2</td></tr>"));
    assert!(content.contains("<hr>"));
}
