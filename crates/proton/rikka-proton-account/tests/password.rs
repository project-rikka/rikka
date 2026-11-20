use crate::support::fixture::{
    EMAIL, PASSWORD, SALT, Server, address, address_key, new_key, ok, passphrase, token, user_key,
};
use rikka_proton_account::{Error, PasswordMode};
use rikka_proton_pgp::{LockedKey, Pgp};
use rikka_test_proton::{lock_key, response};
use serde_json::{Value, json};

const NEW_PASSWORD: &str = "a brand new password";

fn migrated(pgp: &Pgp) -> Server {
    let user = new_key(pgp, "not_for_email_use@domain.tld");
    let original = new_key(pgp, EMAIL);
    let token = token(pgp, &user);

    Server::new(
        vec![user_key(
            "user-key",
            &lock_key(&user, passphrase(PASSWORD, SALT)),
            true,
        )],
        vec![address(
            "address-value",
            EMAIL,
            vec![address_key(
                "address-key",
                &lock_key(&original, &token.secret),
                Some(&token),
                true,
            )],
        )],
    )
}

fn legacy(pgp: &Pgp) -> Server {
    let user = new_key(pgp, "not_for_email_use@domain.tld");
    let original = new_key(pgp, EMAIL);
    let secret = passphrase(PASSWORD, SALT);

    Server::new(
        vec![user_key("user-key", &lock_key(&user, &secret), true)],
        vec![address(
            "address-value",
            EMAIL,
            vec![address_key(
                "address-key",
                &lock_key(&original, &secret),
                None,
                true,
            )],
        )],
    )
}

fn relocked(server: &Server, sent: &Value) {
    let listed = match sent.get("UserKeys") {
        Some(keys) => keys.clone(),
        None => sent["Keys"].clone(),
    };
    let armored = |id: &str| {
        listed
            .as_array()
            .into_iter()
            .flatten()
            .find(|key| key["ID"] == id)
            .and_then(|key| key["PrivateKey"].as_str())
            .map(ToOwned::to_owned)
    };

    server.with(|state| {
        if let Some(user) = armored("user-key") {
            state.user_keys = vec![user_key("user-key", &user, true)];
        }
        if let Some(address) = armored("address-key") {
            state.addresses[0]["Keys"][0]["PrivateKey"] = Value::String(address);
        }
        state.salts = vec![json!({"ID": "user-key", "KeySalt": sent["KeySalt"]})];
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn one_password_change_relocks_user_keys() {
    let pgp = Pgp::new();
    let server = migrated(&pgp);
    let account = server
        .account(|server, request| match request.route() {
            ("PUT", "/core/v4/keys/private") => {
                let sent = request.json();
                let proof = server.prove(&sent);
                relocked(server, &sent);
                Some(response(
                    200,
                    json!({"Code": 1000, "ServerProof": proof}).to_string(),
                ))
            }
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    let changed = account
        .change_password(&keys)
        .password(PASSWORD)
        .new_password(NEW_PASSWORD)
        .execute()
        .await
        .unwrap();

    assert_eq!(changed.unlocked_key_count(), 2);

    let sent = server.last_body("PUT", "/core/v4/keys/private").unwrap();
    assert_eq!(sent["SRPSession"], "srp-session");
    assert!(sent["ClientProof"].is_string());
    assert_eq!(sent["Auth"]["ModulusID"], "modulus-id");
    assert!(sent.get("Keys").is_none());
    assert_eq!(sent["UserKeys"].as_array().map(Vec::len), Some(1));

    let salt = sent["KeySalt"].as_str().unwrap();
    assert_ne!(salt, SALT);
    LockedKey::import(
        sent["UserKeys"][0]["PrivateKey"]
            .as_str()
            .unwrap_or_default(),
    )
    .unwrap()
    .unlock(passphrase(NEW_PASSWORD, salt))
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn mailbox_password_change_relocks_legacy_keys() {
    let pgp = Pgp::new();
    let server = legacy(&pgp);
    let account = server
        .account(|server, request| match request.route() {
            ("PUT", "/core/v4/keys/private") => {
                relocked(server, &request.json());
                Some(ok())
            }
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    let changed = account
        .change_password(&keys)
        .mode(PasswordMode::Two)
        .new_password(NEW_PASSWORD)
        .execute()
        .await
        .unwrap();

    assert_eq!(changed.unlocked_key_count(), 2);

    let sent = server.last_body("PUT", "/core/v4/keys/private").unwrap();
    assert!(sent.get("Auth").is_none());
    assert!(sent.get("ClientProof").is_none());
    assert!(sent.get("UserKeys").is_none());
    assert_eq!(sent["Keys"].as_array().map(Vec::len), Some(2));
}

#[tokio::test(flavor = "multi_thread")]
async fn password_change_rejects_invalid_server_proof() {
    let pgp = Pgp::new();
    let server = migrated(&pgp);
    let account = server
        .account(|_, request| match request.route() {
            ("PUT", "/core/v4/keys/private") => Some(response(
                200,
                json!({"Code": 1000, "ServerProof": "bm90LWEtcHJvb2Y="}).to_string(),
            )),
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    let error = account
        .change_password(&keys)
        .password(PASSWORD)
        .new_password(NEW_PASSWORD)
        .execute()
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        Error::Pgp(rikka_proton_pgp::Error::ServerProof)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn login_password_change_sends_verifier() {
    let server = Server::new(Vec::new(), Vec::new());
    let account = server
        .account(|server, request| match request.route() {
            ("PUT", "/core/v4/settings/password") => {
                let proof = server.prove(&request.json());
                Some(response(
                    200,
                    json!({
                        "Code": 1000,
                        "ServerProof": proof,
                        "UserSettings": {"Password": {"Mode": 2}}
                    })
                    .to_string(),
                ))
            }
            _ => None,
        })
        .await;

    let settings = account
        .change_login_password()
        .password(PASSWORD)
        .totp("123456")
        .new_password(NEW_PASSWORD)
        .execute()
        .await
        .unwrap();

    assert_eq!(settings.password().mode(), PasswordMode::Two);

    let sent = server
        .last_body("PUT", "/core/v4/settings/password")
        .unwrap();
    assert_eq!(sent["TwoFactorCode"], "123456");
    assert_eq!(sent["Auth"]["Version"], 4);
    assert!(sent["Auth"]["Salt"].is_string());
}
