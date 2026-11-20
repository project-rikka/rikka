use rikka_omnibox::{UrlDisplay, copy_text, display_host};
use url::Url;

fn url(text: &str) -> Url {
    Url::parse(text).unwrap()
}

fn emphasis(shown: &UrlDisplay) -> (Option<&str>, Option<&str>) {
    (
        shown.host().map(|range| &shown.text()[range]),
        shown.domain().map(|range| &shown.text()[range]),
    )
}

#[test]
fn shows_steady_urls_like_chrome() {
    let cases = [
        (
            "https://www.example.com/",
            "example.com",
            Some("example.com"),
            Some("example.com"),
        ),
        (
            "http://example.com/",
            "example.com",
            Some("example.com"),
            Some("example.com"),
        ),
        (
            "https://www.example.com/path?q=1#top",
            "example.com/path?q=1#top",
            Some("example.com"),
            Some("example.com"),
        ),
        (
            "https://sub.example.co.uk/a",
            "sub.example.co.uk/a",
            Some("sub.example.co.uk"),
            Some("example.co.uk"),
        ),
        (
            "https://www.co.uk/",
            "www.co.uk",
            Some("www.co.uk"),
            Some("www.co.uk"),
        ),
        (
            "https://user:pw@example.com/",
            "example.com",
            Some("example.com"),
            Some("example.com"),
        ),
        (
            "https://example.com:8443/x",
            "example.com:8443/x",
            Some("example.com"),
            Some("example.com"),
        ),
        (
            "https://user.github.io/blog",
            "user.github.io/blog",
            Some("user.github.io"),
            Some("user.github.io"),
        ),
        (
            "http://192.168.0.1/admin",
            "192.168.0.1/admin",
            Some("192.168.0.1"),
            Some("192.168.0.1"),
        ),
        (
            "http://[::1]:8080/",
            "[::1]:8080",
            Some("[::1]"),
            Some("[::1]"),
        ),
        (
            "http://intranet/wiki",
            "intranet/wiki",
            Some("intranet"),
            Some("intranet"),
        ),
        (
            "https://xn--r8jz45g.jp/",
            "例え.jp",
            Some("例え.jp"),
            Some("例え.jp"),
        ),
        (
            "https://www.xn--wgv71a119e.jp/",
            "日本語.jp",
            Some("日本語.jp"),
            Some("日本語.jp"),
        ),
        (
            "https://xn--80ak6aa92e.com/",
            "xn--80ak6aa92e.com",
            Some("xn--80ak6aa92e.com"),
            Some("xn--80ak6aa92e.com"),
        ),
        (
            "https://ja.wikipedia.org/wiki/%E6%9D%B1%E4%BA%AC",
            "ja.wikipedia.org/wiki/東京",
            Some("ja.wikipedia.org"),
            Some("wikipedia.org"),
        ),
        (
            "https://example.com/a%20b",
            "example.com/a%20b",
            Some("example.com"),
            Some("example.com"),
        ),
        (
            "https://example.com/%E2%80%AEtxt.exe",
            "example.com/%E2%80%AEtxt.exe",
            Some("example.com"),
            Some("example.com"),
        ),
        (
            "file:///home/u/a%20b.html",
            "file:///home/u/a%20b.html",
            None,
            None,
        ),
        ("about:blank", "about:blank", None, None),
        (
            "rikka://settings/search",
            "rikka://settings/search",
            None,
            None,
        ),
        (
            "mailto:someone@example.com",
            "mailto:someone@example.com",
            None,
            None,
        ),
        (
            "view-source:https://www.example.com/",
            "view-source:https://www.example.com/",
            Some("www.example.com"),
            Some("example.com"),
        ),
    ];
    for (input, text, host, domain) in cases {
        let shown = UrlDisplay::steady(&url(input));
        assert_eq!(shown.text(), text, "{input}");
        assert_eq!(emphasis(&shown), (host, domain), "{input}");
    }
}

#[test]
fn guards_against_homographs() {
    let cases = [
        ("аррӏе.com", false),
        ("аpple.com", false),
        ("раураl.com", false),
        ("пример.рф", true),
        ("яндекс.рф", true),
        ("аррӏе.ru", true),
        ("bücher.de", true),
        ("例え.jp", true),
        ("ドメイン名例.jp", true),
        ("ひらがな.com", true),
        ("faß.de", false),
        ("ノ.jp", false),
        ("a\u{30FC}b.jp", false),
        ("中国.cn", true),
        ("한국.kr", true),
        ("ελληνικά.gr", true),
        ("οοο.com", false),
        ("ᎪᏒᎵ.com", false),
    ];
    for (unicode, shown_as_unicode) in cases {
        let ascii = url::quirks::domain_to_ascii(unicode);
        assert!(ascii.contains("xn--"), "{unicode} → {ascii}");
        let shown = display_host(&ascii);
        if shown_as_unicode {
            assert_eq!(shown, url::quirks::domain_to_unicode(&ascii), "{unicode}");
        } else {
            assert!(
                shown.split('.').next().unwrap().starts_with("xn--"),
                "{unicode} → {shown}"
            );
        }
    }
    assert_eq!(display_host("example.com"), "example.com");
    assert_eq!(display_host("xn--invalid-.com"), "xn--invalid-.com");
}

#[test]
fn copies_exact_urls() {
    let address = url("https://www.example.com/wiki/%E6%9D%B1%E4%BA%AC?q=a%20b");
    let steady = UrlDisplay::steady(&address);
    assert_eq!(steady.text(), "example.com/wiki/東京?q=a%20b");
    let whole = 0..steady.text().len();
    assert_eq!(copy_text(&address, steady.text(), whole), address.as_str());
    let full = UrlDisplay::full(&address);
    assert_eq!(
        copy_text(&address, full.text(), 0..full.text().len()),
        address.as_str()
    );
    assert_eq!(
        copy_text(&address, steady.text(), 0..11),
        "https://www.example.com"
    );
    assert_eq!(copy_text(&address, steady.text(), 12..16), "wiki");
    assert_eq!(copy_text(&address, "edited text", 0..6), "edited");
    assert_eq!(copy_text(&address, steady.text(), 0..100), "");
}
