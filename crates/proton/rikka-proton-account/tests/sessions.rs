use std::sync::Arc;

use crate::support::account;
use rikka_proton_account::{Account, Error, ForkKey, ForkPayload};
use rikka_test_proton::{FakeSrp, Request, SIGNED_MODULUS, not_found, response, signed_in_api};
use serde_json::json;

const PASSWORD: &str = "the account password";

#[tokio::test(flavor = "multi_thread")]
async fn sealed_fork_round_trips() {
    let key = ForkKey::generate();
    let sealed = ForkPayload::new()
        .with_key_password("passphrase-value")
        .seal(&key)
        .unwrap();

    let account = account(move |request: Request| {
        let sealed = sealed.clone();

        match request.route() {
            ("GET", "/auth/v4/sessions/forks") => response(
                200,
                json!({"Code": 1000, "Selector": "selector-value", "UserCode": "ABCD1234"})
                    .to_string(),
            ),
            ("POST", "/auth/v4/sessions/forks") => {
                let sent = request.json();
                assert_eq!(sent["ChildClientID"], "cli-pass");
                assert_eq!(sent["Independent"], 1);
                assert_eq!(sent["UserCode"], "ABCD1234");
                assert!(sent["Payload"].is_string());
                response(
                    200,
                    json!({"Code": 1000, "Selector": "selector-value"}).to_string(),
                )
            }
            ("GET", "/auth/v4/sessions/forks/selector-value") => response(
                200,
                json!({
                    "Code": 1000,
                    "UID": "uid-value",
                    "UserID": "user-value",
                    "AccessToken": "access-value",
                    "RefreshToken": "refresh-value",
                    "Scopes": ["self"],
                    "Payload": sealed
                })
                .to_string(),
            ),
            _ => not_found(),
        }
    })
    .await;

    let invitation = account.fork_invitation().await.unwrap();

    assert_eq!(invitation.selector(), "selector-value");
    assert_eq!(
        invitation.sign_in_code(&key, "cli-pass"),
        format!("0:ABCD1234:{}:cli-pass", key.to_base64())
    );

    let selector = account
        .fork()
        .child_client_id("cli-pass")
        .independent(true)
        .user_code(invitation.user_code())
        .sealed_payload(
            ForkPayload::new().with_key_password("passphrase-value"),
            key.clone(),
        )
        .execute()
        .await
        .unwrap();

    let forked = account.forked_session(&selector).await.unwrap();

    assert_eq!(forked.session().uid(), "uid-value");
    assert_eq!(
        forked.payload(&key).unwrap().key_password(),
        Some("passphrase-value")
    );
    assert!(matches!(
        forked.payload(&ForkKey::generate()),
        Err(Error::ForkPayloadDecryption)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_unproven_password_scope() {
    let srp = Arc::new(FakeSrp::with_password(PASSWORD));

    let account = account(move |request: Request| {
        let srp = srp.clone();

        match request.route() {
            ("POST", "/auth/v4/sessions") => crate::support::unauth_session(),
            ("POST", "/auth/v4/info") => response(
                200,
                json!({
                    "Code": 1000,
                    "Modulus": SIGNED_MODULUS,
                    "ServerEphemeral": srp.challenge(),
                    "Version": 4,
                    "Salt": srp.salt(),
                    "SRPSession": "srp-session"
                })
                .to_string(),
            ),
            ("PUT", "/core/v4/users/password") => {
                assert_eq!(request.json()["TwoFactorCode"], "123456");
                response(
                    200,
                    json!({"Code": 1000, "ServerProof": "bm90LWEtcHJvb2Y="}).to_string(),
                )
            }
            _ => not_found(),
        }
    })
    .await;

    let error = account
        .password_scope()
        .username("tester")
        .password(PASSWORD)
        .totp("123456")
        .execute()
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        Error::Pgp(rikka_proton_pgp::Error::ServerProof)
    ));
}

fn session_response() -> rikka_test_proton::ResponseFuture {
    response(
        200,
        json!({
            "Code": 1000,
            "UID": "granted-uid",
            "UserID": "user-value",
            "AccessToken": "access-value",
            "RefreshToken": "refresh-value",
            "Scopes": ["self"]
        })
        .to_string(),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn credentialless_and_mdm_replace_anonymous_session() {
    let server = crate::support::fixture::Server::new(Vec::new(), Vec::new());
    let account = server
        .account(|_, request| match request.route() {
            ("POST", "/auth/v4/credentialless") | ("POST", "/vpn/v1/business/mdm-login") => {
                Some(session_response())
            }
            _ => None,
        })
        .await;

    let session = account.credentialless().execute().await.unwrap();
    assert_eq!(session.uid(), "granted-uid");
    assert_eq!(
        account
            .api()
            .session()
            .map(|stored_session| stored_session.uid().to_owned()),
        Some("granted-uid".to_owned())
    );
    let sent = server.last_body("POST", "/auth/v4/credentialless").unwrap();
    assert!(sent["Payload"]["pass-linux-v4-challenge-0"].is_object());

    account
        .mdm_login("mdm-token", "mdm-group")
        .device("device-unique-id")
        .execute()
        .await
        .unwrap();
    assert_eq!(
        server.last_body("POST", "/vpn/v1/business/mdm-login"),
        Some(json!({
            "MdmToken": "mdm-token",
            "Group": "mdm-group",
            "DeviceUniqId": "device-unique-id"
        }))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_revocation_still_forgets_session() {
    let api = signed_in_api(|request: Request| match request.route() {
        ("DELETE", "/auth/v4") => response(422, r#"{"Code":2000,"Error":"unavailable"}"#),
        _ => not_found(),
    })
    .await;
    let account = Account::new(api);

    assert!(account.revoke_session().await.is_err());
    assert!(account.api().session().is_none());
}
