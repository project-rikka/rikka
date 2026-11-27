use rikka_html::{Document, Escaped};
use rikka_markdown::markdown_text;

fn converted(html: &str) -> String {
    Document::parse(html).to_markdown()
}

fn read_back(html: &str) -> String {
    markdown_text(&converted(html)).to_string()
}

#[test]
fn text_reads_back_verbatim() {
    let samples = [
        "*not bold* _not italic_ `not code` [not](link) <b>not html</b>",
        "# not a heading | not | a table | $not math$ ^not^ ~~not struck~~",
        "&amp; stays escaped {#not-an-id} [[not a wiki]] [^not a note]",
        "1. not a list",
        "2) not a list",
        "- not a bullet",
        "+ not a bullet",
        "> not a quote",
        "= not a heading underline",
        ": not a definition",
        "back\\slash at the end\\",
        "price 1,000円 → 500円 (半額!)",
    ];
    for sample in samples {
        let html = format!("<p>{}</p>", Escaped(sample));
        assert_eq!(read_back(&html), sample, "{html}");
    }
}

#[test]
fn layout_tables_read_as_paragraphs() {
    let html = "<table width=\"100%\"><tr><td>\
        <table><tr><td><img src=\"https://cdn.example/logo.png\" alt=\"logo\"></td></tr>\
        <tr><td>新しいログインがありました<br>EldoDebugさん、ご利用ありがとうございます。</td></tr>\
        <tr><td>&nbsp;</td></tr>\
        <tr><td>時間: 2026年8月14日<br>場所: 日本</td></tr>\
        <tr><td><a href=\"https://www.pixiv.net/history\">ログイン履歴</a>で確認できます。</td></tr>\
        <tr><td><a href=\"https://policies.example/terms\">利用規約</a></td>\
        <td><a href=\"https://policies.example/privacy\">プライバシーポリシー</a></td></tr>\
        </table></td></tr></table>";

    assert_eq!(
        converted(html),
        "新しいログインがありました\\\nEldoDebugさん、ご利用ありがとうございます。\n\n\
         時間: 2026年8月14日\\\n場所: 日本\n\n\
         [ログイン履歴](<https://www.pixiv.net/history>)で確認できます。\n\n\
         [利用規約](<https://policies.example/terms>) · \
         [プライバシーポリシー](<https://policies.example/privacy>)"
    );
}

#[test]
fn structure_converts() {
    let cases = [
        (
            "<h1>Title</h1><p>one <b>bold</b> and <i>slanted</i></p>",
            "# Title\n\none **bold** and *slanted*",
        ),
        (
            "<ul><li>one<li>two<ol start=\"3\"><li>three</ol></ul>",
            "- one\n\n- two\n\n  3. three",
        ),
        ("<ul><li>one</li> <li>two</li></ul>", "- one\n- two"),
        (
            "<blockquote><p>quoted</p><blockquote>deeper</blockquote></blockquote>",
            "> quoted\n>\n> > deeper",
        ),
        (
            "<pre>\nfn main() {\n    ```\n}</pre>",
            "````\nfn main() {\n    ```\n}\n````",
        ),
        ("<p>a<br><br><br><br>b</p>", "a\\\n\\\nb"),
        ("<p>a</p><hr><hr><p>b</p><hr>", "a\n\n***\n\nb"),
        (
            "<table><thead><tr><th>Item</th><th>Price</th></tr></thead>\
             <tr><td>Tea | green</td><td colspan=\"2\">¥300</td></tr></table>",
            "| Item | Price |  |\n| --- | --- | --- |\n| Tea \\| green | ¥300 |  |",
        ),
        (
            "<table><tr><th>A</th><th>B</th><th>C</th></tr>\
             <tr><td rowspan=\"2\">a</td><td>b1</td><td>c1</td></tr>\
             <tr><td>b2</td><td>c2</td></tr></table>",
            "| A | B | C |\n| --- | --- | --- |\n| a | b1 | c1 |\n|  | b2 | c2 |",
        ),
        (
            "<ul><li>a</ul><ul><li>b</ul><ul><li>c</ul><ol><li>d</ol><ol start=\"5\"><li>e</ol>",
            "- a\n\n* b\n\n- c\n\n1. d\n\n5) e",
        ),
        (
            "<p><span style=\"font-weight: 700\">heavy</span> <s>gone</s> <code>a`b</code></p>",
            "**heavy** ~~gone~~ ``a`b``",
        ),
        ("<p><b>bold </b>text</p>", "**bold** text"),
        ("<p>重要<b>「注意」</b>です</p>", "重要「注意」です"),
    ];
    for (html, expected) in cases {
        assert_eq!(converted(html), expected, "{html}");
    }
}

#[test]
fn links_keep_only_web_and_mail_targets() {
    let cases = [
        (
            "<a href=\" https://example.com/a b?x=<y> \">site</a>",
            "[site](<https://example.com/a%20b?x=%3Cy%3E>)",
        ),
        (
            "<a href=\"mailto:help@example.com\">mail us</a>",
            "[mail us](<mailto:help@example.com>)",
        ),
        ("<a href=\"javascript:alert(1)\">run</a>", "run"),
        ("<a href=\"/relative\">here</a>", "here"),
        (
            "<a href=\"https://example.com\"><div><b>Open</b></div></a>",
            "[**Open**](<https://example.com>)",
        ),
        (
            "<a href=\"https://example.com\"><img src=\"https://t.example/b.png\" alt=\"Track order\"></a>",
            "[Track order](<https://example.com>)",
        ),
        (
            "Wow!<a href=\"https://example.com\">site</a>",
            "Wow\\![site](<https://example.com>)",
        ),
        (
            "<table><tr><th>Link</th><th>N</th></tr>\
             <tr><td><a href=\"https://a.example/?u=a|b\">x</a></td><td>1</td></tr></table>",
            "| Link | N |\n| --- | --- |\n| [x](<https://a.example/?u=a%7Cb>) | 1 |",
        ),
    ];
    for (html, expected) in cases {
        assert_eq!(converted(html), expected, "{html}");
    }
}

#[test]
fn hidden_and_remote_content_is_dropped() {
    let html = "<html><head><title>t</title><style>p{color:red}</style></head><body>\
        <div style=\"display:none; max-height:0\">preheader&zwnj;&nbsp;&zwnj;</div>\
        <span hidden>secret</span><p style=\"visibility: hidden !important\">ghost</p>\
        <img src=\"https://tracker.example/pixel.gif\" alt=\"pixel\">\
        <script>alert(1)</script><p>body</p></body></html>";
    let deep = format!(
        "{}<style>p{{color:red}}</style><span hidden>secret</span>deep{}",
        "<span>".repeat(400),
        "</span>".repeat(400)
    );

    assert_eq!(converted(html), "body");
    assert_eq!(converted(&deep), "deep");
}

#[test]
fn text_keeps_link_targets() {
    let html = "<h2>Order</h2><p>Price: <b>$5</b> &amp; <a href=\"https://example.com/o\">details</a><br>\
        <a href=\"https://example.com/\">https://example.com/</a> \
        <a href=\"mailto:help@example.com\">help@example.com</a></p>\
        <ul><li>one<li>two</ul><blockquote>quoted</blockquote>";

    assert_eq!(
        Document::parse(html).to_text(),
        "Order\n\nPrice: $5 & details <https://example.com/o>\n\
         https://example.com/ help@example.com\n\n- one\n- two\n\n> quoted"
    );
}

#[test]
fn deep_nesting_is_flattened() {
    let depth = 1_000;
    let html = format!(
        "{}deep{}",
        "<blockquote><div>".repeat(depth),
        "</div></blockquote>".repeat(depth)
    );

    assert!(read_back(&html).contains("deep"));
}

#[test]
fn located_pictures_become_image_blocks() {
    let html = "<p>Before <img src=\"cid:logo\" alt=\"Logo [new]\"> after</p>\
        <img src=\"https://example.com/pixel.gif\" width=\"1\" height=\"1\">\
        <blockquote><img src=\"https://example.com/far.png\" alt=\"Remote chart\"></blockquote>\
        <a href=\"https://example.com/sale\"><img src=\"cid:banner\" alt=\"Sale\"></a>\
        <a href=\"https://example.com/far\"><img src=\"https://example.com/far.png\" alt=\"Far\"></a>\
        <table><tr><th>Name</th><th>Count</th></tr>\
        <tr><td><a href=\"https://example.com/cell\"><img src=\"cid:cell\" alt=\"Cell\"></a></td><td>1</td></tr></table>\
        <img src=\"https://example.com/unnamed.png\">";
    let located = |source: &str| {
        source
            .strip_prefix("cid:")
            .map(|name| format!("/cache/{name} (1).png"))
    };

    assert_eq!(
        Document::parse(html).to_markdown_with_pictures(located),
        "Before\n\n![Logo \\[new\\]](</cache/logo%20(1).png>)\n\nafter\n\n> Remote chart\n\n\
        [![Sale](</cache/banner%20(1).png>)](<https://example.com/sale>)\n\n\
        [Far](<https://example.com/far>)\n\n\
        | Name | Count |\n| --- | --- |\n| [Cell](<https://example.com/cell>) | 1 |"
    );
    assert_eq!(
        converted(html),
        "Before after\n\n[Sale](<https://example.com/sale>)[Far](<https://example.com/far>)\n\n\
        | Name | Count |\n| --- | --- |\n| [Cell](<https://example.com/cell>) | 1 |"
    );
}
