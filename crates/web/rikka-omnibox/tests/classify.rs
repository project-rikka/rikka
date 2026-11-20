use rikka_omnibox::{Intent, SearchEngine, SearchEngines, Upgrade, classify};
use url::Url;

#[derive(Debug, Clone, Copy)]
enum Expect {
    Navigate(&'static str, Upgrade),
    Search(&'static str),
    SearchOr(&'static str, &'static str),
    Empty,
}

fn engines() -> SearchEngines {
    let mut engines = SearchEngines::builtin();
    engines
        .add(
            SearchEngine::builder("wikipedia-ja")
                .name("Wikipedia")
                .keyword("w")
                .search_url("https://ja.wikipedia.org/wiki/Special:Search?search={searchTerms}")
                .build()
                .unwrap(),
        )
        .unwrap();
    engines
}

fn check(input: &str, expected: Expect) {
    let engines = engines();
    let classified = classify(input, &engines);
    match expected {
        Expect::Empty => assert!(classified.is_empty(), "{input:?}: {classified:?}"),
        Expect::Navigate(url, upgrade) => {
            let Intent::Navigate(destination) = classified.intent() else {
                panic!("{input:?} should navigate: {classified:?}");
            };
            assert_eq!(destination.url().as_str(), url, "{input:?}");
            assert_eq!(destination.upgrade(), upgrade, "{input:?}");
            assert!(classified.alternative().is_none(), "{input:?}");
        }
        Expect::Search(terms) | Expect::SearchOr(terms, _) => {
            let search = classified
                .search()
                .unwrap_or_else(|| panic!("{input:?} should search: {classified:?}"));
            assert_eq!(search.terms(), terms, "{input:?}");
            let alternative = classified
                .alternative()
                .map(|destination| destination.url().as_str());
            let expected = match expected {
                Expect::SearchOr(_, url) => Some(url),
                _ => None,
            };
            assert_eq!(alternative, expected, "{input:?}");
        }
    }
}

#[test]
fn classifies_real_world_inputs() {
    use Expect::*;
    use Upgrade::*;
    let cases = [
        ("", Empty),
        ("   ", Empty),
        ("?", Empty),
        ("example.com", Navigate("https://example.com/", Assumed)),
        ("  Example.COM  ", Navigate("https://example.com/", Assumed)),
        (
            "www.example.co.jp/path?q=1#top",
            Navigate("https://www.example.co.jp/path?q=1#top", Assumed),
        ),
        (
            "example.com:8443/admin",
            Navigate("https://example.com:8443/admin", Assumed),
        ),
        (
            "example.com/a b",
            Navigate("https://example.com/a%20b", Assumed),
        ),
        (
            "user.github.io",
            Navigate("https://user.github.io/", Assumed),
        ),
        ("github.io", Navigate("https://github.io/", Assumed)),
        ("example.com.", Navigate("https://example.com./", Assumed)),
        (
            "https://example.com",
            Navigate("https://example.com/", NotApplicable),
        ),
        (
            "http://example.com",
            Navigate("http://example.com/", Available),
        ),
        (
            "HTTP://Example.com/Path",
            Navigate("http://example.com/Path", Available),
        ),
        (
            "http:example.com",
            Navigate("http://example.com/", Available),
        ),
        (
            "http://localhost:8080",
            Navigate("http://localhost:8080/", Exempt),
        ),
        (
            "http://192.168.1.1",
            Navigate("http://192.168.1.1/", Exempt),
        ),
        ("localhost", Navigate("http://localhost/", Exempt)),
        (
            "localhost:3000/api",
            Navigate("http://localhost:3000/api", Exempt),
        ),
        ("app.localhost", Navigate("http://app.localhost/", Exempt)),
        ("printer.local", Navigate("http://printer.local/", Exempt)),
        (
            "router.home.arpa/setup",
            Navigate("http://router.home.arpa/setup", Exempt),
        ),
        ("192.168.0.1", Navigate("http://192.168.0.1/", Exempt)),
        ("1.1.1.1", Navigate("http://1.1.1.1/", Exempt)),
        (
            "10.0.0.2:8080/x",
            Navigate("http://10.0.0.2:8080/x", Exempt),
        ),
        ("[::1]:8080", Navigate("http://[::1]:8080/", Exempt)),
        ("::1", Navigate("http://[::1]/", Exempt)),
        ("fe80::1", Navigate("http://[fe80::1]/", Exempt)),
        ("nas:5000", Navigate("http://nas:5000/", Exempt)),
        (
            "user:secret@example.com",
            Navigate("https://user:secret@example.com/", Assumed),
        ),
        ("例え.jp", Navigate("https://xn--r8jz45g.jp/", Assumed)),
        ("例え。jp", Navigate("https://xn--r8jz45g.jp/", Assumed)),
        (
            "日本語.jp/パス",
            Navigate("https://xn--wgv71a119e.jp/%E3%83%91%E3%82%B9", Assumed),
        ),
        (
            "ｇｏｏｇｌｅ．ｃｏｍ",
            Navigate("https://google.com/", Assumed),
        ),
        ("bücher.de", Navigate("https://xn--bcher-kva.de/", Assumed)),
        (
            "пример.рф",
            Navigate("https://xn--e1afmkfd.xn--p1ai/", Assumed),
        ),
        (
            "file:///etc/hosts",
            Navigate("file:///etc/hosts", NotApplicable),
        ),
        ("about:blank", Navigate("about:blank", NotApplicable)),
        ("about:", Navigate("about:blank", NotApplicable)),
        (
            "about:settings",
            Navigate("rikka://settings", NotApplicable),
        ),
        ("about:history", Navigate("rikka://history", NotApplicable)),
        (
            "rikka://settings/search",
            Navigate("rikka://settings/search", NotApplicable),
        ),
        (
            "rikka:downloads",
            Navigate("rikka://downloads", NotApplicable),
        ),
        (
            "RIKKA://Archive",
            Navigate("rikka://archive", NotApplicable),
        ),
        (
            "data:text/plain,hello",
            Navigate("data:text/plain,hello", NotApplicable),
        ),
        (
            "blob:https://example.com/550e8400-e29b-41d4-a716-446655440000",
            Navigate(
                "blob:https://example.com/550e8400-e29b-41d4-a716-446655440000",
                NotApplicable,
            ),
        ),
        (
            "view-source:example.com",
            Navigate("view-source:https://example.com/", NotApplicable),
        ),
        (
            "view-source:http://example.com/x",
            Navigate("view-source:http://example.com/x", NotApplicable),
        ),
        (
            "mailto:someone@example.com",
            Navigate("mailto:someone@example.com", NotApplicable),
        ),
        (
            "magnet:?xt=urn:btih:abc",
            Navigate("magnet:?xt=urn:btih:abc", NotApplicable),
        ),
        (
            "/home/user/page.html",
            Navigate("file:///home/user/page.html", NotApplicable),
        ),
        (
            "/tmp/a b#c.html",
            Navigate("file:///tmp/a%20b%23c.html", NotApplicable),
        ),
        ("rust", SearchOr("rust", "http://rust/")),
        (
            "intranet/wiki",
            SearchOr("intranet/wiki", "http://intranet/wiki"),
        ),
        ("node.js", SearchOr("node.js", "http://node.js/")),
        (
            "server.corp",
            SearchOr("server.corp", "http://server.corp/"),
        ),
        ("rust lang", Search("rust lang")),
        ("what is rust?", Search("what is rust?")),
        ("what?", Search("what?")),
        ("c++", Search("c++")),
        ("3.14", Search("3.14")),
        ("2024", Search("2024")),
        ("1.2.3", Search("1.2.3")),
        ("foo_bar", Search("foo_bar")),
        ("co.jp", Search("co.jp")),
        ("東京.jp", Search("東京.jp")),
        ("東京", Search("東京")),
        ("東京 天気", Search("東京 天気")),
        ("東京　天気", Search("東京　天気")),
        ("ｒｕｓｔ", Search("ｒｕｓｔ")),
        ("こんにちは。", Search("こんにちは。")),
        ("「東京」とは", Search("「東京」とは")),
        ("\"example.com\"", Search("\"example.com\"")),
        ("?example.com", Search("example.com")),
        ("? rust", Search("rust")),
        ("someone@example.com", Search("someone@example.com")),
        ("example.com search", Search("example.com search")),
        ("foo:bar", Search("foo:bar")),
        ("note: buy milk", Search("note: buy milk")),
        ("localhost:", Search("localhost:")),
        ("example.com:99999", Search("example.com:99999")),
        ("javascript:alert(1)", Search("javascript:alert(1)")),
        (
            "JaVaScRiPt:alert(document.cookie)",
            Search("JaVaScRiPt:alert(document.cookie)"),
        ),
        ("  javascript:void(0)", Search("javascript:void(0)")),
        (
            "view-source:javascript:alert(1)",
            Search("view-source:javascript:alert(1)"),
        ),
        ("https://", Search("https://")),
        ("http://foo bar", Search("http://foo bar")),
        ("C:\\Windows", Search("C:\\Windows")),
    ];
    for (input, expected) in cases {
        check(input, expected);
    }
}

#[test]
fn keywords_pick_their_engine() {
    let engines = engines();
    let cases = [
        (
            "@ddg rust",
            "duckduckgo",
            "rust",
            "https://duckduckgo.com/?q=rust",
        ),
        (
            "@DDG  rust lang",
            "duckduckgo",
            "rust lang",
            "https://duckduckgo.com/?q=rust+lang",
        ),
        (
            "@yahoo 東京",
            "yahoo-japan",
            "東京",
            "https://search.yahoo.co.jp/search?p=%E6%9D%B1%E4%BA%AC&ei=UTF-8",
        ),
        (
            "w 東京都",
            "wikipedia-ja",
            "東京都",
            "https://ja.wikipedia.org/wiki/Special:Search?search=%E6%9D%B1%E4%BA%AC%E9%83%BD",
        ),
    ];
    for (input, engine, terms, url) in cases {
        let classified = classify(input, &engines);
        let search = classified.search().expect(input);
        assert_eq!(search.engine(), engine, "{input}");
        assert_eq!(search.terms(), terms, "{input}");
        assert_eq!(search.url().as_str(), url, "{input}");
        assert!(search.by_keyword(), "{input}");
    }
    let plain = classify("@ddg", &engines);
    assert_eq!(plain.search().map(|search| search.engine()), Some("google"));
    let default = classify("rust lang", &engines);
    assert!(!default.search().unwrap().by_keyword());
    assert_eq!(
        default.search().unwrap().url().as_str(),
        "https://www.google.com/search?q=rust+lang&ie=UTF-8"
    );
}

#[test]
fn suggestion_terms_only_for_safe_inputs() {
    let engines = SearchEngines::builtin();
    let cases = [
        ("rust lang", Some("rust lang")),
        ("example.com", Some("example.com")),
        ("example.com/private/path", None),
        ("https://example.com", None),
        ("user:pw@example.com", None),
        ("192.168.0.1", None),
        ("/home/user/a.html", None),
        ("", None),
        ("@ddg 東京", Some("東京")),
    ];
    for (input, expected) in cases {
        assert_eq!(
            classify(input, &engines).suggestion_terms(),
            expected,
            "{input}"
        );
    }
}

#[test]
fn typed_host_falls_back_to_http() {
    let engines = SearchEngines::builtin();
    let classified = classify("example.com/x", &engines);
    assert_eq!(
        classified
            .destination()
            .and_then(|destination| destination.http_fallback())
            .as_ref()
            .map(Url::as_str),
        Some("http://example.com/x")
    );
    let explicit = classify("https://example.com/x", &engines);
    assert_eq!(explicit.destination().unwrap().http_fallback(), None);
}
