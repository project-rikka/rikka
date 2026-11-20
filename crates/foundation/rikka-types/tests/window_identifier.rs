use rikka_types::{ParseWindowIdentifierError, WindowIdentifier};

#[test]
fn identifiers_follow_portal_format() {
    let cases = [
        (
            "wayland:4f2a91",
            Ok(WindowIdentifier::Wayland("4f2a91".into())),
        ),
        ("x11:3a00004", Ok(WindowIdentifier::X11(0x3a0_0004))),
        ("x11:0x3a00004", Ok(WindowIdentifier::X11(0x3a0_0004))),
        ("win32:1a0c2e", Ok(WindowIdentifier::Win32(0x1a_0c2e))),
        ("win32:0x1a0c2e", Ok(WindowIdentifier::Win32(0x1a_0c2e))),
        ("wayland:", Err(ParseWindowIdentifierError::Handle)),
        ("x11:", Err(ParseWindowIdentifierError::Window)),
        ("x11:0", Err(ParseWindowIdentifierError::Window)),
        ("x11:zz", Err(ParseWindowIdentifierError::Window)),
        ("win32:0", Err(ParseWindowIdentifierError::Window)),
        ("", Err(ParseWindowIdentifierError::Kind)),
        ("mir:1", Err(ParseWindowIdentifierError::Kind)),
    ];
    for (text, parsed) in cases {
        assert_eq!(text.parse::<WindowIdentifier>(), parsed, "{text}");
    }
    assert_eq!(WindowIdentifier::X11(0x3a0_0004).to_string(), "x11:3a00004");
    assert_eq!(
        WindowIdentifier::Wayland("4f2a91".into()).to_string(),
        "wayland:4f2a91"
    );
    assert_eq!(
        WindowIdentifier::Win32(0x1a_0c2e).to_string(),
        "win32:1a0c2e"
    );
}
