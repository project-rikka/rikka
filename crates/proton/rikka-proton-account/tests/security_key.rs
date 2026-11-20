use std::sync::{Arc, Mutex};

use rikka_fido::Fido;
use rikka_proton_account::{Account, LoginOutcome};
use rikka_test_fido::{FakeKey, Kind};
use serde_json::{Value, json};

use crate::support::account;
use rikka_test_proton::{FakeSrp, Request, ResponseFuture, SIGNED_MODULUS, not_found, response};

const PASSWORD: &str = "the account password";
const FIDO2_ONLY: i32 = 2;

fn credential_id() -> Vec<u8> {
    (0u8..64).map(|at| at.wrapping_mul(7)).collect()
}

fn authentication_options() -> Value {
    json!({
        "publicKey": {
            "challenge": (0u8..32).collect::<Vec<u8>>(),
            "timeout": 60000,
            "rpId": "proton.me",
            "allowCredentials": [
                {"type": "public-key", "id": credential_id(), "transports": ["usb"]}
            ],
            "userVerification": "discouraged",
            "extensions": {"appid": "https://proton.me"}
        }
    })
}

fn second_factor() -> Value {
    json!({
        "Enabled": FIDO2_ONLY,
        "FIDO2": {
            "AuthenticationOptions": authentication_options(),
            "RegisteredKeys": [
                {"AttestationFormat": "packed", "CredentialID": credential_id(), "Name": "Key"}
            ]
        }
    })
}

async fn signing_in(posted: Arc<Mutex<Option<Value>>>) -> Account {
    let srp = Arc::new(FakeSrp::with_password(PASSWORD));

    account(move |request: Request| -> ResponseFuture {
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
                    "SRPSession": "srp-session",
                    "2FA": second_factor()
                })
                .to_string(),
            ),
            ("POST", "/auth/v4") => {
                let sent = request.json();
                let proof = srp
                    .prove(
                        sent["ClientEphemeral"].as_str().unwrap_or_default(),
                        sent["ClientProof"].as_str().unwrap_or_default(),
                    )
                    .unwrap();
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
                        "ServerProof": proof,
                        "PasswordMode": 1,
                        "2FA": second_factor(),
                        "TemporaryPassword": 0
                    })
                    .to_string(),
                )
            }
            ("POST", "/auth/v4/2fa") => {
                *posted.lock().unwrap() = Some(request.json());
                response(
                    200,
                    json!({"Code": 1000, "Scopes": ["self", "user", "password"]}).to_string(),
                )
            }
            ("GET", "/core/v4/users") => response(
                200,
                json!({"Code": 1000, "User": {"ID": "user-value", "Name": "tester", "Keys": []}})
                    .to_string(),
            ),
            ("GET", "/core/v4/addresses") => response(
                200,
                json!({"Code": 1000, "Addresses": [], "Total": 0}).to_string(),
            ),
            _ => not_found(),
        }
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn security_key_answers_fido2_challenge() {
    let posted = Arc::new(Mutex::new(None));
    let account = signing_in(posted.clone()).await;

    let LoginOutcome::TwoFactorRequired(two_factor) = account
        .login()
        .username("tester")
        .password(PASSWORD)
        .execute()
        .await
        .unwrap()
    else {
        panic!("expected second factor prompt");
    };

    let request = two_factor.security_key_request().unwrap();
    assert_eq!(request.rp_id(), "proton.me");
    assert_eq!(request.origin(), "https://account.proton.me");

    let key = FakeKey::new(Kind::Ctap21).with_credential("proton.me", credential_id());
    let assertion = Fido::with_transport(key)
        .assert(request)
        .unwrap()
        .finish()
        .await
        .unwrap();

    two_factor
        .security_key(&assertion)
        .await
        .unwrap()
        .into_unlocked()
        .unwrap();

    let sent = posted.lock().unwrap().take().unwrap();
    let fido2 = &sent["FIDO2"];
    assert_eq!(fido2["AuthenticationOptions"], authentication_options());
    assert_eq!(fido2["CredentialID"], json!(credential_id()));

    let decode =
        |field: &str| rikka_proton_api::base64::decode(fido2[field].as_str().unwrap()).unwrap();
    let client_data: Value = serde_json::from_slice(&decode("ClientData")).unwrap();
    assert_eq!(client_data["type"], "webauthn.get");
    assert_eq!(client_data["origin"], "https://account.proton.me");
    assert_eq!(client_data["crossOrigin"], false);
    assert_eq!(
        client_data["challenge"],
        "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"
    );

    let authenticator_data = decode("AuthenticatorData");
    assert_eq!(
        authenticator_data[..32],
        rikka_proton_pgp::sha256("proton.me")
    );
    assert_eq!(decode("Signature"), assertion.signature());
    assert!(sent.get("TwoFactorCode").is_none());
}
