use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rikka_oauth::Token;
use serde_json::{Value, json};

fn id_token(claims: &str) -> String {
    format!("header.{}.signature", URL_SAFE_NO_PAD.encode(claims))
}

fn token(stored: Value) -> Token {
    serde_json::from_value(stored).expect("a stored token")
}

#[test]
fn stored_token_round_trips() {
    let stored = json!({
        "access_token": "notarealtoken",
        "refresh_token": "notarealtoken",
        "id_token": id_token(r#"{"sub":"somebody"}"#),
        "expires_at": 1_900_000_000_u64,
    });
    let read = token(stored.clone());

    assert_eq!(read.identity().text(&["sub"]), "somebody");
    assert_eq!(read.expires_at(), Some(1_900_000_000));
    assert_eq!(serde_json::to_value(&read).expect("storable"), stored);

    assert!(serde_json::from_value::<Token>(json!({ "refresh_token": "notarealtoken" })).is_err());
    assert!(serde_json::from_value::<Token>(json!({ "access_token": "" })).is_err());
}

#[test]
fn debug_redacts_token() {
    let read = token(json!({
        "access_token": "notarealtoken",
        "refresh_token": "notarealtoken",
        "id_token": id_token(r#"{"email":"notarealtoken"}"#),
    }));

    assert!(!format!("{read:?}").contains("notarealtoken"));
}
