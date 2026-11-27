use browser_pages::{Internal, Section};

#[test]
fn internal_addresses_round_trip() {
    let cases = [
        ("rikka://home", Internal::Home),
        ("rikka://settings", Internal::Settings(None)),
        (
            "rikka://settings/chrome",
            Internal::Settings(Some(Section::Chrome)),
        ),
        (
            "rikka://settings/clear-browsing-data",
            Internal::Settings(Some(Section::ClearData)),
        ),
        ("rikka://history", Internal::History),
        ("rikka://downloads", Internal::Downloads),
        ("rikka://extensions", Internal::Extensions),
    ];
    for (address, internal) in cases {
        assert_eq!(
            Internal::parse(address).as_ref(),
            Some(&internal),
            "{address}"
        );
        assert_eq!(internal.address(), address);
    }
    assert_eq!(Internal::parse("view-source:https://example.com/"), None);
    for missing in ["rikka://nowhere", "rikka://settings/https-only"] {
        assert_eq!(
            Internal::parse(missing),
            Some(Internal::Missing(missing.to_owned()))
        );
    }
    assert_eq!(Internal::parse("https://example.com/"), None);
    assert_eq!(Internal::parse("chrome://settings"), None);
    assert_eq!(Section::ClearData.listed_under(), Section::Privacy);
}
