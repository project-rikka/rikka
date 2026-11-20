use rikka_network::{Invitation, Security, Ssid};

fn invitation(name: &str, security: Security, passphrase: &str, hidden: bool) -> Invitation {
    Invitation {
        ssid: Ssid::from(name),
        security,
        passphrase: passphrase.to_owned(),
        hidden,
    }
}

#[test]
fn share_qr_payload_matches_camera_format() {
    for (shared, encoded) in [
        (
            invitation("Rikka", Security::Personal, "hunter2", false),
            "WIFI:T:WPA;S:Rikka;P:hunter2;;",
        ),
        (
            invitation("Guests", Security::Open, "", true),
            "WIFI:T:nopass;S:Guests;H:true;;",
        ),
        (
            invitation("a;b", Security::Wpa3Personal, "c:d\\e", false),
            "WIFI:T:SAE;S:a\\;b;P:c\\:d\\\\e;;",
        ),
        (
            invitation("cafe", Security::Personal, "abc", false),
            "WIFI:T:WPA;S:\"cafe\";P:abc;;",
        ),
    ] {
        assert_eq!(shared.to_string(), encoded);
        assert_eq!(
            encoded.parse::<Invitation>().ok(),
            Some(shared),
            "{encoded}"
        );
    }
}

#[test]
fn parses_codes_from_other_devices() {
    for (input, expected) in [
        (
            "WIFI:S:Home Net;T:WPA;P:pass word;H:false;;",
            invitation("Home Net", Security::Personal, "pass word", false),
        ),
        (
            "wifi:t:wpa2;s:Cafe;p:latte;;",
            invitation("Cafe", Security::Personal, "latte", false),
        ),
        (
            "WIFI:T:WPA;R:1;S:Modern;P:secret123;;",
            invitation("Modern", Security::Wpa3Personal, "secret123", false),
        ),
        (
            "WIFI:T:WEP;S:Old;P:\"0123456789\";H:TRUE;;",
            invitation("Old", Security::Wep, "0123456789", true),
        ),
        (
            "WIFI:S:Lobby;;",
            invitation("Lobby", Security::Open, "", false),
        ),
        (
            "WIFI:S:NoType;P:only-a-password;;",
            invitation("NoType", Security::Personal, "only-a-password", false),
        ),
        (
            "WIFI:T:WPA2-EAP;S:Campus;E:PEAP;I:me;P:pw;;",
            invitation("Campus", Security::Enterprise, "pw", false),
        ),
        (
            "  WIFI:S:\\\"Quoted\\\";T:SAE;P:\\;\\,;;trailing",
            invitation("\"Quoted\"", Security::Wpa3Personal, ";,", false),
        ),
    ] {
        assert_eq!(input.parse::<Invitation>().ok(), Some(expected), "{input}");
    }
}

#[test]
fn rejects_unjoinable_codes() {
    for input in [
        "https://example.com",
        "WIFI:T:WPA;P:nameless;;",
        "WIFI:T:WPA;S:Locked;;",
        "WIFI:T:ROT13;S:Odd;P:x;;",
        "WIFI:S:this name is far longer than thirty-two bytes;;",
    ] {
        assert!(input.parse::<Invitation>().is_err(), "{input}");
    }
}
