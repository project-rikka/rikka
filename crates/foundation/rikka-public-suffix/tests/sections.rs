use rikka_public_suffix::{Section, icann_suffix, is_public_suffix, site, suffix};

#[test]
fn suffixes_report_their_section() {
    let cases = [
        (
            "www.example.com",
            Some(("com", Section::Icann)),
            Some("com"),
        ),
        (
            "user.github.io",
            Some(("github.io", Section::Private)),
            Some("io"),
        ),
        (
            "a.b.c.kobe.jp",
            Some(("c.kobe.jp", Section::Icann)),
            Some("c.kobe.jp"),
        ),
        ("www.ck", Some(("ck", Section::Icann)), Some("ck")),
        ("host.unlisted", Some(("unlisted", Section::Unlisted)), None),
        (
            "EXAMPLE.CO.UK",
            Some(("CO.UK", Section::Icann)),
            Some("CO.UK"),
        ),
        ("bad..domain", None, None),
        ("", None, None),
    ];
    for (domain, expected, icann) in cases {
        assert_eq!(
            suffix(domain).map(|suffix| (suffix.as_str(), suffix.section())),
            expected,
            "{domain}"
        );
        assert_eq!(
            icann_suffix(domain).map(|suffix| suffix.as_str()),
            icann,
            "{domain}"
        );
    }
}

#[test]
fn public_suffixes_and_sites() {
    let cases = [
        ("co.uk", true, "co.uk"),
        ("github.io", true, "github.io"),
        ("example.co.uk", false, "example.co.uk"),
        ("a.b.example.co.uk", false, "example.co.uk"),
        ("unlisted", false, "unlisted"),
        ("a.b.unlisted", false, "b.unlisted"),
    ];
    for (domain, public, expected) in cases {
        assert_eq!(is_public_suffix(domain), public, "{domain}");
        assert_eq!(site(domain), expected, "{domain}");
    }
}
