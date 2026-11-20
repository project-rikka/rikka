use browser_pages::{Origin, covers, registrable_site, web_host, web_site};

#[test]
fn hosts_and_sites_come_from_web_addresses_only() {
    let cases = [
        (
            "https://WWW.Shop.Example.co.uk./cart?x=1",
            Some("www.shop.example.co.uk"),
            Some("shop.example.co.uk"),
            Some("example.co.uk"),
        ),
        (
            "http://localhost:8080/",
            Some("localhost"),
            Some("localhost"),
            Some("localhost"),
        ),
        (
            "https://192.168.0.1/",
            Some("192.168.0.1"),
            Some("192.168.0.1"),
            Some("192.168.0.1"),
        ),
        ("file:///home/a/page.html", None, None, None),
        ("about:blank", None, None, None),
        ("not a url", None, None, None),
    ];
    for (url, host, site, registrable) in cases {
        assert_eq!(web_host(url).as_deref(), host, "{url}");
        assert_eq!(web_site(url).as_deref(), site, "{url}");
        assert_eq!(registrable_site(url).as_deref(), registrable, "{url}");
    }
}

#[test]
fn a_domain_covers_itself_and_its_subdomains_only() {
    let cases = [
        ("example.com", "example.com", true),
        ("example.com", "mail.example.com", true),
        ("example.com", "badexample.com", false),
        ("example.com", "example.com.evil.test", false),
        ("mail.example.com", "example.com", false),
        ("", "example.com", false),
    ];
    for (domain, host, expected) in cases {
        assert_eq!(covers(domain, host), expected, "{domain} over {host}");
    }
}

#[test]
fn origins_keep_scheme_host_and_explicit_port() {
    let cases = [
        ("https://Example.com./a/b?c#d", Some("https://example.com")),
        (
            "https://example.com:8443/",
            Some("https://example.com:8443"),
        ),
        ("http://[::1]:8080/x", Some("http://[::1]:8080")),
        (
            "moz-extension://2f3a/page.html",
            Some("moz-extension://2f3a"),
        ),
        ("about:blank", None),
        ("file:///home/a", None),
    ];
    for (uri, expected) in cases {
        assert_eq!(
            Origin::parse(uri)
                .map(|origin| origin.to_string())
                .as_deref(),
            expected,
            "{uri}"
        );
    }
    assert_eq!(
        Origin::parse("https://a.example/one"),
        Origin::parse("https://A.example:443/two")
    );
}
