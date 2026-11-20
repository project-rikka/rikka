use rikka_html::{Document, Selector};

fn texts(document: &Document, selector: &str) -> Vec<String> {
    let selector = Selector::parse(selector).expect("selector parses");
    document
        .select(&selector)
        .map(|element| element.text().collect::<String>())
        .collect()
}

#[test]
fn omitted_end_tags_are_implied() {
    let cases = [
        (
            "<ul><li>one<li>two<ul><li>inner</ul><li>three</ul>",
            "ul > li",
            vec!["one", "twoinner", "inner", "three"],
        ),
        ("<p>one<p>two<div>three</div>four", "p", vec!["one", "two"]),
        (
            "<dl><dt>term<dd>one<dd>two</dl>",
            "dl > dd",
            vec!["one", "two"],
        ),
        (
            "<table><tr><td>a<td>b<tr><td>c</table>",
            "tr > td",
            vec!["a", "b", "c"],
        ),
        (
            "<select><option>a<option>b</select>",
            "select > option",
            vec!["a", "b"],
        ),
        (
            "<div><img src=x>after<br>more</div>",
            "div",
            vec!["aftermore"],
        ),
        ("<h2>title<h3>next</h3>", "h3", vec!["next"]),
    ];
    for (html, selector, expected) in cases {
        assert_eq!(texts(&Document::parse(html), selector), expected, "{html}");
    }
}

#[test]
fn stray_end_tags_do_not_close_their_surroundings() {
    let cases = [
        ("<div><span>a</div>b</span>c", "div", vec!["a"]),
        ("<div>a</span>b</div>", "div", vec!["ab"]),
        (
            "<table><tr><td><p>a</div>b</td></tr></table>",
            "td",
            vec!["ab"],
        ),
        ("<section><a>x</section>y</a>", "section", vec!["x"]),
    ];
    for (html, selector, expected) in cases {
        assert_eq!(texts(&Document::parse(html), selector), expected, "{html}");
    }
}

#[test]
fn scripts_and_foreign_content_keep_the_tree_intact() {
    let html = "<div id=a><script>document.write('<div>')</script>\
                <svg><path d=m0/><circle/></svg><p>shown</p></div><div id=b>next</div>";
    let document = Document::parse(html);
    assert_eq!(texts(&document, "#a > p"), ["shown"]);
    assert_eq!(texts(&document, "#b"), ["next"]);
    assert_eq!(texts(&document, "svg > circle"), [""]);
    assert_eq!(
        document.text().collect::<String>(),
        "document.write('<div>')shownnext"
    );
}
