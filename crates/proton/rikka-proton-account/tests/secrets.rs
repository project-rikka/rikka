use rikka_proton_account::{ForkKey, ForkPayload, SessionInfo};

const SIGN_IN_RESPONSE: &str = r#"{
    "Code": 1000,
    "AccessToken": "access-value",
    "TokenType": "Bearer",
    "Scopes": ["self", "user"],
    "UID": "uid-value",
    "UserID": "user-value",
    "RefreshToken": "refresh-value",
    "EventID": "event-value",
    "LocalID": 3,
    "ServerProof": "proof-value",
    "PasswordMode": 2,
    "2FA": {"Enabled": 1, "FIDO2": {"AuthenticationOptions": null, "RegisteredKeys": []}},
    "TemporaryPassword": 1
}"#;

#[test]
fn fork_payload_requires_sealing_key() {
    let key = ForkKey::generate();
    let payload = ForkPayload::new().with_key_password("passphrase-value");
    let sealed = payload.seal(&key).unwrap();
    assert!(!sealed.contains("passphrase-value"));

    let decrypted = ForkPayload::open(&key, &sealed).unwrap();
    assert_eq!(decrypted.key_password(), Some("passphrase-value"));
    assert!(ForkPayload::open(&ForkKey::generate(), &sealed).is_err());
    assert!(ForkPayload::open(&key, "AAAA").is_err());
}

#[test]
fn debug_output_redacts_secrets() {
    let info: SessionInfo = serde_json::from_str(SIGN_IN_RESPONSE).unwrap();
    let debug = format!("{info:?}");
    assert!(!debug.contains("access-value"), "{debug}");
    assert!(!debug.contains("refresh-value"), "{debug}");

    let payload = ForkPayload::new().with_key_password("passphrase-value");
    assert!(!format!("{payload:?}").contains("passphrase-value"));

    let key = ForkKey::from_bytes([7u8; 32]).unwrap();
    assert!(!format!("{key:?}").contains('7'));
}
