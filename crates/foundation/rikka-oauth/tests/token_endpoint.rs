use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rikka_oauth::{Client, Error, Loopback};
use serde_json::{Value, json};

use crate::support::{form, serve};

#[tokio::test]
async fn exchanges_code_with_verifier_and_secret() {
    let (endpoint, request) = serve(
        "200 OK",
        json!({ "access_token": "fresh", "refresh_token": "kept", "expires_in": 3600 }),
    )
    .await;
    let client = Client::builder("rikka", endpoint)
        .secret("notasecret")
        .build()
        .expect("a client");
    let redirect = Loopback::localhost(0)
        .path("/callback")
        .bind()
        .await
        .expect("a free port");
    let authorization = client
        .authorization("https://auth.example.com/authorize", redirect.uri())
        .pkce()
        .build()
        .expect("an authorization");

    let token = client
        .exchange("thecode", &authorization)
        .send()
        .await
        .expect("the code was exchanged");

    let request = request.await.expect("the server ran");
    let sent = form(&request.body);
    assert_eq!(request.headers["accept"], "application/json");
    assert_eq!(sent["grant_type"], "authorization_code");
    assert_eq!(sent["client_id"], "rikka");
    assert_eq!(sent["client_secret"], "notasecret");
    assert_eq!(sent["code"], "thecode");
    assert_eq!(sent["redirect_uri"], redirect.uri());
    assert_eq!(sent["code_verifier"].len(), 128);
    assert_eq!(token.access_token(), "fresh");
    assert_eq!(token.refresh_token(), Some("kept"));
}

#[tokio::test]
async fn refresh_keeps_unrotated_refresh_token() {
    let (endpoint, request) = serve("200 OK", json!({ "access_token": "renewed" })).await;
    let client = Client::builder("rikka", endpoint)
        .json()
        .header("user-agent", "rikka-test")
        .build()
        .expect("a client");

    let token = client
        .refresh("old")
        .field("scope", "openid")
        .send()
        .await
        .expect("the token was renewed");

    let request = request.await.expect("the server ran");
    let sent: Value = serde_json::from_str(&request.body).expect("a JSON body");
    assert_eq!(request.headers["user-agent"], "rikka-test");
    assert_eq!(
        sent,
        json!({
            "client_id": "rikka",
            "grant_type": "refresh_token",
            "refresh_token": "old",
            "scope": "openid",
        })
    );
    assert_eq!(token.access_token(), "renewed");
    assert_eq!(token.refresh_token(), Some("old"));
}

#[tokio::test]
async fn basic_secret_travels_in_header_only() {
    let (endpoint, request) = serve("200 OK", json!({ "access_token": "renewed" })).await;
    let client = Client::builder("rikka app", endpoint)
        .secret("not/a:secret")
        .basic()
        .build()
        .expect("a client");

    client
        .refresh("old")
        .send()
        .await
        .expect("the token was renewed");

    let request = request.await.expect("the server ran");
    let sent = form(&request.body);
    assert_eq!(
        request.headers["authorization"],
        format!("Basic {}", STANDARD.encode("rikka+app:not%2Fa%3Asecret"))
    );
    assert_eq!(sent["client_id"], "rikka app");
    assert!(!sent.contains_key("client_secret"));
}

#[tokio::test]
async fn reports_invalid_grant() {
    let (endpoint, _) = serve(
        "400 Bad Request",
        json!({ "error": "invalid_grant", "error_description": "revoked" }),
    )
    .await;
    let client = Client::new("rikka", endpoint).expect("a client");

    let refused = client.refresh("spent").send().await;

    assert!(matches!(refused, Err(Error::ServerResponse(refusal)) if refusal.is_expired()));
}
