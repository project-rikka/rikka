use music_core::Isrc;

#[test]
fn isrc_parses_hyphens_and_case() {
    let plain = Isrc::parse("USUM71703861").unwrap();

    assert_eq!(Isrc::parse("us-um7-17-03861"), Some(plain.clone()));
    assert_eq!(plain.as_str(), "USUM71703861");
}

#[test]
fn malformed_isrc_is_rejected() {
    for malformed in [
        "USUM7170386",
        "USUM717038612",
        "1SUM71703861",
        "USUM7A703861",
        "",
    ] {
        assert_eq!(Isrc::parse(malformed), None, "{malformed}");
    }
}
