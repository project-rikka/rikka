use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use rikka_proton_vpn::VpnClient;
use serde_json::json;

use rikka_test_proton::{Record, Request, api, not_found, response};

fn session(uid: &str) -> serde_json::Value {
    json!({
        "Code": 1000,
        "UID": uid,
        "AccessToken": format!("{uid}-access"),
        "RefreshToken": format!("{uid}-refresh"),
        "TokenType": "Bearer",
        "Scopes": ["vpn"]
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn sign_in_code_awaits_fork() {
    let polls = Arc::new(AtomicUsize::new(0));
    let polled = polls.clone();
    let api = api(move |request: Request| match request.route() {
        ("POST", "/auth/v4/sessions") => response(200, session("anonymous").to_string()),
        ("GET", "/auth/v4/sessions/forks") => response(
            200,
            r#"{"Code":1000,"Selector":"selector-one","UserCode":"ABCD1234"}"#,
        ),
        ("GET", "/auth/v4/sessions/forks/selector-one") => {
            match polled.fetch_add(1, Ordering::SeqCst) {
                0 => response(422, r#"{"Code":2501,"Error":"Not yet"}"#),
                _ => {
                    let payload = rikka_proton_api::base64::encode(
                        br#"{"InitialUserTier":"plus","FlowType":"tv"}"#,
                    );
                    let mut forked = session("forked");
                    forked["Payload"] = json!(payload);
                    response(200, forked.to_string())
                }
            }
        }
        _ => not_found(),
    })
    .await;
    let client = VpnClient::new(api);

    let code = client.sign_in_code().await.unwrap();
    assert_eq!(code.user_code(), "ABCD1234");
    assert_eq!(
        code.url("linux-vpn"),
        "https://account.proton.me/vpn/tv/code/ABCD1234?clientId=linux-vpn"
    );
    assert!(code.expires_at() > jiff::Timestamp::now());

    assert!(client.poll_sign_in(&code).await.unwrap().is_none());
    let signed_in = client.poll_sign_in(&code).await.unwrap().unwrap();

    assert_eq!(signed_in.session.uid(), "forked");
    let payload = signed_in.payload.unwrap();
    assert_eq!(payload.initial_user_tier, "plus");
    assert_eq!(payload.flow_type, "tv");
    assert_eq!(
        client
            .api()
            .session()
            .map(|session| session.uid().to_owned()),
        Some("forked".to_owned())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn web_sign_in_uses_fork_selector() {
    let record = Record::default();
    let noted = record.clone();
    let api = api(move |request: Request| {
        noted.push(&request);
        match request.route() {
            ("POST", "/auth/v4/sessions/forks") => {
                response(200, r#"{"Code":1000,"Selector":"web-selector"}"#)
            }
            _ => not_found(),
        }
    })
    .await;

    let url = VpnClient::new(api)
        .web_sign_in_url("https://account.protonvpn.com/dashboard#old")
        .await
        .unwrap();

    assert_eq!(
        url,
        "https://account.protonvpn.com/dashboard#selector=web-selector"
    );
    let request = record.matching("POST", "/auth/v4/sessions/forks")[0].json();
    assert_eq!(request["ChildClientID"], json!("web-account-lite"));
    assert_eq!(request["Payload"], json!(""));
    assert_eq!(request["Independent"], json!(0));
}
