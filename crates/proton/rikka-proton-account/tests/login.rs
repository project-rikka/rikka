use std::sync::Arc;

use crate::support::account;
use rikka_proton_account::{Account, Error, LoginOutcome};
use rikka_test_proton::{FakeSrp, Request, ResponseFuture, SIGNED_MODULUS, not_found, response};
use serde_json::json;

const PASSWORD: &str = "the account password";

fn auth_info_response(srp: &FakeSrp, second_factor: i32, sso: Option<&str>) -> ResponseFuture {
    if let Some(token) = sso {
        return response(
            200,
            json!({"Code": 1000, "SSOChallengeToken": token}).to_string(),
        );
    }

    response(
        200,
        json!({
            "Code": 1000,
            "Modulus": SIGNED_MODULUS,
            "ServerEphemeral": srp.challenge(),
            "Version": 4,
            "Salt": srp.salt(),
            "SRPSession": "srp-session",
            "2FA": {"Enabled": second_factor, "FIDO2": {"RegisteredKeys": []}}
        })
        .to_string(),
    )
}

fn session_response(server_proof: &str, password_mode: i32, second_factor: i32) -> ResponseFuture {
    response(
        200,
        json!({
            "Code": 1000,
            "UID": "uid-value",
            "UserID": "user-value",
            "AccessToken": "access-value",
            "RefreshToken": "refresh-value",
            "TokenType": "Bearer",
            "Scopes": ["self"],
            "EventID": "event-value",
            "LocalID": 0,
            "ServerProof": server_proof,
            "PasswordMode": password_mode,
            "2FA": {"Enabled": second_factor, "FIDO2": {"RegisteredKeys": []}},
            "TemporaryPassword": 0
        })
        .to_string(),
    )
}

fn empty_account(request: &Request) -> Option<ResponseFuture> {
    match request.route() {
        ("GET", "/core/v4/users") => Some(response(
            200,
            json!({"Code": 1000, "User": {"ID": "user-value", "Name": "tester", "Keys": []}})
                .to_string(),
        )),
        ("GET", "/core/v4/addresses") => Some(response(
            200,
            json!({"Code": 1000, "Addresses": [], "Total": 0}).to_string(),
        )),
        _ => None,
    }
}

async fn signing_in(password_mode: i32, second_factor: i32) -> Account {
    let srp = Arc::new(FakeSrp::with_password(PASSWORD));

    account(move |request: Request| {
        let srp = srp.clone();

        match request.route() {
            ("POST", "/auth/v4/sessions") => crate::support::unauth_session(),
            ("POST", "/auth/v4/info") => auth_info_response(&srp, second_factor, None),
            ("POST", "/auth/v4") => {
                let sent = request.json();
                let proof = srp
                    .prove(
                        sent["ClientEphemeral"].as_str().unwrap_or_default(),
                        sent["ClientProof"].as_str().unwrap_or_default(),
                    )
                    .unwrap();
                session_response(&proof, password_mode, second_factor)
            }
            ("POST", "/auth/v4/2fa") => response(
                200,
                json!({"Code": 1000, "Scopes": ["self", "user", "password"]}).to_string(),
            ),
            _ => empty_account(&request).unwrap_or_else(not_found),
        }
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn one_password_login_unlocks_keys() {
    let account = signing_in(1, 0).await;

    let outcome = account
        .login()
        .username("tester")
        .password(PASSWORD)
        .execute()
        .await
        .unwrap();

    let unlocked = outcome.into_unlocked().unwrap();

    assert_eq!(unlocked.session().uid(), "uid-value");
    assert_eq!(unlocked.session().event_id(), Some("event-value"));
    assert_eq!(unlocked.keys().user().name(), Some("tester"));
    assert_eq!(unlocked.keys().unlocked_key_count(), 0);
    assert_eq!(account.api().session().unwrap().uid(), "uid-value");
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_invalid_server_proof() {
    let srp = Arc::new(FakeSrp::with_password(PASSWORD));

    let account = account(move |request: Request| {
        let srp = srp.clone();

        match request.route() {
            ("POST", "/auth/v4/sessions") => crate::support::unauth_session(),
            ("POST", "/auth/v4/info") => auth_info_response(&srp, 0, None),
            ("POST", "/auth/v4") => session_response("bm90LWEtcHJvb2Y=", 1, 0),
            _ => not_found(),
        }
    })
    .await;

    let error = account
        .login()
        .username("tester")
        .password(PASSWORD)
        .execute()
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        Error::Pgp(rikka_proton_pgp::Error::ServerProof)
    ));
    let stored_session = account.api().session().unwrap();
    assert_eq!(stored_session.uid(), "unauth-session");
}

#[tokio::test(flavor = "multi_thread")]
async fn second_factor_pauses_login() {
    let account = signing_in(1, 1).await;

    let outcome = account
        .login()
        .username("tester")
        .password(PASSWORD)
        .execute()
        .await
        .unwrap();

    let LoginOutcome::TwoFactorRequired(two_factor) = outcome else {
        panic!("expected second factor prompt");
    };

    assert!(two_factor.has_totp());
    assert!(!two_factor.has_fido2());

    let unlocked = two_factor
        .totp("123456")
        .await
        .unwrap()
        .into_unlocked()
        .unwrap();

    assert!(unlocked.session().has_scope("password"));
    assert!(account.api().session().unwrap().has_scope("password"));
}

#[tokio::test(flavor = "multi_thread")]
async fn two_password_login_requires_mailbox_password() {
    let account = signing_in(2, 0).await;

    let outcome = account
        .login()
        .username("tester")
        .password(PASSWORD)
        .execute()
        .await
        .unwrap();

    let LoginOutcome::MailboxPasswordRequired(mailbox) = outcome else {
        panic!("expected mailbox password prompt");
    };

    assert_eq!(mailbox.session().uid(), "uid-value");

    let unlocked = mailbox.submit("the mailbox password").await.unwrap();

    assert_eq!(unlocked.keys().user().id().as_str(), "user-value");
}

#[tokio::test(flavor = "multi_thread")]
async fn empty_username_sends_no_request() {
    let account = account(|_: Request| not_found()).await;

    let error = account
        .login()
        .password(PASSWORD)
        .execute()
        .await
        .unwrap_err();

    assert!(matches!(error, Error::MissingUsername));
}
