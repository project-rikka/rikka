use rikka_oauth::{Error, Registration};
use serde_json::{Value, json};

use crate::support::serve;

#[tokio::test]
async fn registers_public_native_client() {
    let (endpoint, request) = serve(
        "201 Created",
        json!({ "client_id": "issued", "client_secret": "" }),
    )
    .await;

    let registered = Registration::new("Rikka")
        .redirect_uri("http://localhost:4455/callback")
        .scope("read write")
        .send(&endpoint)
        .await
        .expect("the client was registered");

    let request = request.await.expect("the server ran");
    let sent: Value = serde_json::from_str(&request.body).expect("a JSON body");
    assert_eq!(request.headers["content-type"], "application/json");
    assert_eq!(
        sent,
        json!({
            "client_name": "Rikka",
            "redirect_uris": ["http://localhost:4455/callback"],
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
            "token_endpoint_auth_method": "none",
            "application_type": "native",
            "scope": "read write",
        })
    );
    assert_eq!(registered.client_id(), "issued");
    assert_eq!(registered.client_secret(), None);
}

#[tokio::test]
async fn refused_registration_reports_server_error() {
    let (endpoint, _) = serve(
        "400 Bad Request",
        json!({ "error": "invalid_redirect_uri", "error_description": "not allowed" }),
    )
    .await;

    let refused = Registration::new("Rikka")
        .redirect_uri("http://localhost:4455/callback")
        .send(&endpoint)
        .await;

    assert!(
        matches!(&refused, Err(Error::ServerResponse(refusal)) if refusal.code() == "invalid_redirect_uri"),
        "{refused:?}"
    );
}
