use crate::support::fixture::{
    EMAIL, PASSWORD, SALT, Server, address, address_key, new_key, ok, passphrase, token, user_key,
};
use rikka_proton_account::{
    AuthDevice, AuthDeviceError, AuthDeviceId, AuthDeviceState, ConfirmationCodeVersion,
    DeviceSecret, Error,
};
use rikka_proton_pgp::{Pgp, PrivateKey, SymmetricKey};
use rikka_test_proton::{lock_key, response};
use serde_json::{Value, json};

const SECRET: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
const CONTEXT: &[u8] = b"account.device-secret";

fn seal_passphrase(secret: &str, passphrase: &[u8]) -> String {
    let key = SymmetricKey::from_bytes(rikka_proton_api::base64::decode(secret).unwrap()).unwrap();

    rikka_proton_api::base64::encode(key.seal(passphrase, CONTEXT).unwrap())
}

fn open_passphrase(secret: &str, sealed: &str) -> Vec<u8> {
    let key = SymmetricKey::from_bytes(rikka_proton_api::base64::decode(secret).unwrap()).unwrap();

    key.open(rikka_proton_api::base64::decode(sealed).unwrap(), CONTEXT)
        .unwrap()
}

struct Fixture {
    server: Server,
    address_key: PrivateKey,
}

fn generate_fixture(pgp: &Pgp) -> Fixture {
    let user = new_key(pgp, "not_for_email_use@domain.tld");
    let original = new_key(pgp, EMAIL);
    let token = token(pgp, &user);

    Fixture {
        server: Server::new(
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
        ),
        address_key: original,
    }
}

#[test]
fn confirmation_codes_match_both_versions() {
    let secret = DeviceSecret::new(AuthDeviceId::from("device-id"), "token", SECRET);

    assert_eq!(
        secret.confirmation_code(ConfirmationCodeVersion::HexDigest),
        "6MRK"
    );
    assert_eq!(
        secret.confirmation_code(ConfirmationCodeVersion::Digest),
        "A5J3"
    );
    assert!(!format!("{secret:?}").contains(SECRET));
}

#[tokio::test(flavor = "multi_thread")]
async fn new_device_seals_secret_to_primary_key() {
    let pgp = Pgp::new();
    let original = generate_fixture(&pgp);
    let account = original
        .server
        .account(|_, request| match request.route() {
            ("POST", "/auth/v4/devices") => Some(response(
                200,
                json!({
                    "Code": 1000,
                    "AuthDevice": {
                        "ID": "device-id",
                        "State": 2,
                        "Name": "Rikka",
                        "LocalizedClientName": "Rikka Pass",
                        "Platform": "Linux",
                        "CreateTime": 1_700_000_000,
                        "LastActivityTime": 1_700_000_000,
                        "DeviceToken": "device-token"
                    }
                })
                .to_string(),
            )),
            _ => None,
        })
        .await;

    let created = account.create_auth_device("Rikka").await.unwrap();

    assert_eq!(created.device().state(), AuthDeviceState::PendingActivation);
    assert!(created.device().is_pending_activation());
    assert_eq!(created.secret().device().as_str(), "device-id");
    assert_eq!(created.secret().token().expose(), "device-token");
    assert_eq!(created.confirmation_code().len(), 4);

    let sent = original
        .server
        .last_body("POST", "/auth/v4/devices")
        .unwrap();
    assert_eq!(sent["Name"], "Rikka");
    assert_eq!(sent["ConfirmationCodeVersion"], 1);

    let decrypted = pgp
        .decrypt()
        .with_key(original.address_key.clone())
        .message(sent["ActivationToken"].as_str().unwrap_or_default())
        .unwrap();
    assert_eq!(
        decrypted.data(),
        created.secret().secret().expose().as_bytes()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn associated_device_unlocks_keys() {
    let pgp = Pgp::new();
    let original = generate_fixture(&pgp);
    let account = original
        .server
        .account(|_, request| match request.route() {
            ("POST", "/auth/v4/devices/device-id/associate") => {
                assert_eq!(request.json()["DeviceToken"], "device-token");
                Some(response(
                    200,
                    json!({
                        "Code": 1000,
                        "AuthDevice": {
                            "ID": "device-id",
                            "EncryptedSecret": seal_passphrase(SECRET, &passphrase(PASSWORD, SALT))
                        }
                    })
                    .to_string(),
                ))
            }
            _ => None,
        })
        .await;

    let secret = DeviceSecret::new(AuthDeviceId::from("device-id"), "device-token", SECRET);
    let keys = account.unlock_with_device(&secret).await.unwrap();

    assert_eq!(keys.unlocked_key_count(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn rejected_association_reports_reason() {
    let cases = [
        (10_300, AuthDeviceError::NotFound),
        (10_301, AuthDeviceError::NotActive),
        (10_302, AuthDeviceError::TokenInvalid),
        (10_303, AuthDeviceError::Rejected),
    ];

    for (code, expected) in cases {
        let server = Server::new(Vec::new(), Vec::new());
        let account = server
            .account(move |_, request| match request.route() {
                ("POST", "/auth/v4/devices/device-id/associate") => Some(response(
                    422,
                    json!({"Code": code, "Error": "refused"}).to_string(),
                )),
                _ => None,
            })
            .await;

        let secret = DeviceSecret::new(AuthDeviceId::from("device-id"), "device-token", SECRET);
        let error = account.device_passphrase(&secret).await.unwrap_err();

        assert!(
            matches!(error, Error::AuthDevice { reason, .. } if reason == expected),
            "{code}: {error}"
        );
    }
}

fn pending(pgp: &Pgp, address_key: &PrivateKey) -> AuthDevice {
    let token = pgp
        .encrypt()
        .to(address_key.public_key())
        .as_utf8()
        .armored_message(SECRET)
        .unwrap();

    serde_json::from_value(json!({
        "ID": "other-device",
        "State": 2,
        "Name": "Laptop",
        "LocalizedClientName": "Rikka Pass",
        "CreateTime": 1,
        "LastActivityTime": 1,
        "ActivationToken": token,
        "ActivationAddressID": "address-value",
        "ConfirmationCodeVersion": 2
    }))
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn device_approval_requires_confirmation_code() {
    let pgp = Pgp::new();
    let original = generate_fixture(&pgp);
    let account = original
        .server
        .account(|_, request| match request.route() {
            ("POST", "/auth/v4/devices/other-device") => Some(ok()),
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();
    let device = pending(&pgp, &original.address_key);

    let error = account
        .approve_auth_device(&keys, &device, "ZZZZ")
        .await
        .unwrap_err();
    assert!(matches!(error, Error::WrongConfirmationCode(_)));
    assert!(
        original
            .server
            .last_body("POST", "/auth/v4/devices/other-device")
            .is_none()
    );

    account
        .approve_auth_device(&keys, &device, "a5j3")
        .await
        .unwrap();

    let sent: Value = original
        .server
        .last_body("POST", "/auth/v4/devices/other-device")
        .unwrap();
    assert_eq!(
        open_passphrase(SECRET, sent["EncryptedSecret"].as_str().unwrap_or_default()),
        passphrase(PASSWORD, SALT)
    );
}
