use code_meaning::{Caution, Meaning, meaning};
use rikka_network::Security;

#[test]
fn links_resolve_to_real_target() {
    for (text, address, host, cautions) in [
        (
            "https://example.com/menu?table=4",
            "https://example.com/menu?table=4",
            "example.com",
            &[][..],
        ),
        (
            "HTTP://EXAMPLE.COM/A",
            "http://example.com/A",
            "example.com",
            &[Caution::Unencrypted][..],
        ),
        (
            "https://bank.example@evil.example/login",
            "https://bank.example@evil.example/login",
            "evil.example",
            &[Caution::Credentials][..],
        ),
        (
            "https://аpple.com/",
            "https://xn--pple-43d.com/",
            "xn--pple-43d.com",
            &[Caution::ForeignLetters][..],
        ),
        (
            "  http://192.168.1.1:8080/  ",
            "http://192.168.1.1:8080/",
            "192.168.1.1",
            &[Caution::Unencrypted, Caution::NumericHost][..],
        ),
    ] {
        let Meaning::Link(link) = meaning(text) else {
            panic!("`{text}` is not read as a link");
        };
        assert_eq!(link.address(), address, "{text}");
        assert_eq!(link.host(), host, "{text}");
        assert_eq!(link.cautions(), cautions, "{text}");
    }
}

#[test]
fn only_browser_urls_are_links() {
    for text in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "mailto:someone@example.com",
        "https://example.com and more words",
        "www.example.com",
        "4006381333931",
        "WIFI:T:WPA;S:Locked;;",
    ] {
        assert_eq!(meaning(text), Meaning::Text, "{text}");
    }
}

#[test]
fn shared_network_parses_as_one() {
    let Meaning::Network(invitation) = meaning("WIFI:T:SAE;S:Rikka;P:hunter2;;") else {
        panic!("the network is not read as one");
    };
    assert_eq!(invitation.ssid.name(), "Rikka");
    assert_eq!(invitation.security, Security::Wpa3Personal);
    assert_eq!(invitation.passphrase, "hunter2");
}
