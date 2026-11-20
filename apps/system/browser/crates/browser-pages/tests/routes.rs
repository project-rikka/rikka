use browser_pages::{Internal, Section};

#[test]
fn internal_addresses_round_trip() {
    let cases = [
        ("rikka://home", Internal::Home),
        ("rikka://settings", Internal::Settings(None)),
        (
            "rikka://settings/https-only",
            Internal::Settings(Some(Section::HttpsOnly)),
        ),
        (
            "rikka://settings/clear-browsing-data",
            Internal::Settings(Some(Section::ClearData)),
        ),
        ("rikka://history", Internal::History),
        ("rikka://downloads", Internal::Downloads),
        (
            "rikka://source?url=https%3A%2F%2Fexample.com%2F%3Fq%3D1",
            Internal::Source("https://example.com/?q=1".to_owned()),
        ),
    ];
    for (address, internal) in cases {
        assert_eq!(
            Internal::parse(address).as_ref(),
            Some(&internal),
            "{address}"
        );
        assert_eq!(internal.address(), address);
    }
    assert_eq!(
        Internal::parse("view-source:https://example.com/"),
        Some(Internal::Source("https://example.com/".to_owned()))
    );
    assert_eq!(
        Internal::parse("rikka://nowhere"),
        Some(Internal::Missing("rikka://nowhere".to_owned()))
    );
    assert_eq!(Internal::parse("https://example.com/"), None);
    assert_eq!(Section::ClearData.listed_under(), Section::Privacy);
}
