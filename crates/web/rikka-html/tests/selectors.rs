use rikka_html::{Document, Selector};

const PAGE: &str = "\
<div id=main class=\"results wide\">\
  <div class=\"result first\" data-url=\"https://one.example/\"><h2><a href=/one>One</a></h2>\
    <p class=snippet>about <b>one</b></p></div>\
  <div class=\"result ad\"><h2><a>Ad</a></h2></div>\
  <section><div class=result><h2><a href=/two title=Second>Two</a></h2>\
    <span class=\"meta date\">today</span></div></section>\
  <form action=/search><input type=submit value=Next><input name=q value=rust>\
    <input type=hidden name=page value=2></form>\
</div>";

fn selected(selector: &str) -> Vec<String> {
    let document = Document::parse(PAGE);
    let selector = Selector::parse(selector).expect("selector parses");
    document
        .select(&selector)
        .map(|element| {
            let text = element.text().collect::<String>();
            match text.trim().is_empty() {
                true => element.attribute("name").unwrap_or_default().to_owned(),
                false => text.trim().to_owned(),
            }
        })
        .collect()
}

#[test]
fn selectors_pick_elements_in_document_order() {
    let cases = [
        ("h2 a", vec!["One", "Ad", "Two"]),
        ("H2 > A[HREF]", vec!["One", "Two"]),
        ("#main > div.result:not(.ad) a", vec!["One"]),
        ("div.result.first a, section a", vec!["One", "Two"]),
        ("[data-url] b", vec!["one"]),
        ("a[title='Second']", vec!["Two"]),
        ("[class*='et'] b, span.meta.date", vec!["one", "today"]),
        ("input[name]:not([type='submit'])", vec!["q", "page"]),
        (
            "form[action=\"/search\"] > input[type=hidden]",
            vec!["page"],
        ),
        ("* > section > div", vec!["Twotoday"]),
        ("div.result > a", vec![]),
        ("#main div.results", vec![]),
    ];
    for (selector, expected) in cases {
        assert_eq!(selected(selector), expected, "{selector}");
    }
}

#[test]
fn element_selection_skips_the_element_and_sees_outer_ancestors() {
    let document = Document::parse(PAGE);
    let result = Selector::parse("div.result").expect("selector parses");
    let nested = Selector::parse("#main div").expect("selector parses");
    let link = Selector::parse("section h2 > a").expect("selector parses");
    let results: Vec<_> = document.select(&result).collect();
    assert_eq!(results.len(), 3);
    assert_eq!(results[0].select(&nested).count(), 0);
    assert_eq!(results[0].select(&link).count(), 0);
    assert_eq!(
        results[2]
            .select(&link)
            .filter_map(|link| link.attribute("href"))
            .collect::<Vec<_>>(),
        ["/two"]
    );
    assert_eq!(
        results[2].parent().map(|parent| parent.name()),
        Some("section")
    );
}

#[test]
fn malformed_selectors_are_refused() {
    for selector in [
        "",
        "div >",
        "a,",
        ".",
        "[href",
        "a:hover",
        "a[b^='c']",
        "a ! b",
    ] {
        assert!(Selector::parse(selector).is_err(), "{selector}");
    }
}
