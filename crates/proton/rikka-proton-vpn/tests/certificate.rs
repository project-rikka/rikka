use jiff::Timestamp;
use rikka_proton_vpn::{Certificate, Freshness};

const PAYLOAD: &str = r#"{
    "Code": 1000,
    "SerialNumber": "0x7b",
    "ClientKeyFingerprint": "fingerprint-value",
    "ClientKey": "-----BEGIN PUBLIC KEY-----\nkey\n-----END PUBLIC KEY-----\n",
    "Certificate": "-----BEGIN CERTIFICATE-----\ncert\n-----END CERTIFICATE-----\n",
    "ExpirationTime": 1700000000,
    "RefreshTime": 1699000000,
    "Mode": "session",
    "DeviceName": "rikka",
    "ServerPublicKeyMode": "EC",
    "ServerPublicKey": "-----BEGIN PUBLIC KEY-----\nserver\n-----END PUBLIC KEY-----\n"
}"#;

fn timestamp(second: i64) -> Timestamp {
    Timestamp::from_second(second).unwrap()
}

#[test]
fn certificate_freshness_deadlines() {
    let certificate: Certificate = serde_json::from_str(PAYLOAD).unwrap();
    assert!(certificate.matches("fingerprint-value"));
    assert!(!certificate.matches("another"));

    for (second, freshness) in [
        (1_698_000_000, Freshness::Fresh),
        (1_699_000_000, Freshness::Stale),
        (1_699_999_999, Freshness::Stale),
        (1_700_000_000, Freshness::Expired),
    ] {
        assert_eq!(
            certificate.freshness_at(timestamp(second)),
            freshness,
            "{second}"
        );
    }
}
