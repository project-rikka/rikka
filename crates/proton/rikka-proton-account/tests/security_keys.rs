use std::sync::{Arc, Mutex, MutexGuard};

use base64::prelude::{BASE64_URL_SAFE_NO_PAD, Engine as _};
use rikka_fido::Fido;
use rikka_fido::cbor::Value as Cbor;
use rikka_proton_account::{Account, Error, SecurityKeyChallenge};
use rikka_test_fido::{FakeKey, Kind};
use serde_json::{Value, json};

use crate::support::account;
use rikka_test_proton::{FakeSrp, Request, ResponseFuture, SIGNED_MODULUS, not_found, response};

const PASSWORD: &str = "the account password";
const PROTON_RP: &str = "proton.me";

#[derive(Debug, Default)]
struct Backend {
    scoped: bool,
    locks: usize,
    keys: Vec<Value>,
    registered: Option<Value>,
}

#[derive(Debug, Clone, Default)]
struct Shared(Arc<Mutex<Backend>>);

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Backend> {
        self.0.lock().unwrap()
    }
}

fn registration_options() -> Value {
    json!({
        "publicKey": {
            "rp": {"id": PROTON_RP, "name": "Proton"},
            "user": {"id": [7, 7, 7, 7], "name": "tester@proton.me", "displayName": "Tester"},
            "challenge": (100u8..132).collect::<Vec<u8>>(),
            "pubKeyCredParams": [
                {"type": "public-key", "alg": -7},
                {"type": "public-key", "alg": -257}
            ],
            "timeout": 60000,
            "excludeCredentials": [{"type": "public-key", "id": [9, 9, 9, 9]}],
            "authenticatorSelection": {"requireResidentKey": false, "userVerification": "discouraged"},
            "attestation": "direct",
            "extensions": {"credProps": true}
        }
    })
}

fn key_index(backend: &Backend, encoded: &str) -> Option<usize> {
    let id = BASE64_URL_SAFE_NO_PAD.decode(encoded).ok()?;
    backend
        .keys
        .iter()
        .position(|key| key["CredentialID"] == json!(id))
}

fn respond(backend: &Shared, srp: &FakeSrp, request: &Request) -> ResponseFuture {
    let ok = || response(200, json!({"Code": 1000}).to_string());
    let refused = || response(403, json!({"Code": 9101, "Error": "scope"}).to_string());
    let route = request.route();

    match route {
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
            let sent = request.json();
            let proof = srp.prove(
                sent["ClientEphemeral"].as_str().unwrap_or_default(),
                sent["ClientProof"].as_str().unwrap_or_default(),
            )
.unwrap();
            backend.lock().scoped = true;
            response(200, json!({"Code": 1000, "ServerProof": proof}).to_string())
        }
        ("PUT", "/core/v4/users/lock") => {
            let mut state = backend.lock();
            state.scoped = false;
            state.locks += 1;
            ok()
        }
        ("GET", "/core/v4/settings") => response(
            200,
            json!({
                "Code": 1000,
                "UserSettings": {"2FA": {"Enabled": 2, "Allowed": 3, "RegisteredKeys": backend.lock().keys}}
            })
            .to_string(),
        ),
        ("GET", "/core/v4/settings/2fa/register") if backend.lock().scoped => response(
            200,
            json!({
                "Code": 1000,
                "RegistrationOptions": registration_options(),
                "RegisteredKeys": backend.lock().keys,
                "AttestationFormats": ["packed", "fido-u2f", "none"]
            })
            .to_string(),
        ),
        ("POST", "/core/v4/settings/2fa/register") if backend.lock().scoped => {
            let sent = request.json();
            let attestation = rikka_proton_api::base64::decode(
                sent["AttestationObject"].as_str().unwrap_or_default(),
            )
            .unwrap_or_default();
            let object = Cbor::decode(&attestation).unwrap_or(Cbor::Null);
            let auth_data = object
                .get("authData")
                .and_then(Cbor::as_bytes)
                .unwrap_or_default();
            let id_length = auth_data
                .get(53..55)
                .map_or(0, |length| usize::from(u16::from_be_bytes([length[0], length[1]])));
            let id = auth_data.get(55..55 + id_length).unwrap_or_default().to_vec();

            let mut state = backend.lock();
            state.keys.push(json!({
                "AttestationFormat": object.get("fmt").and_then(Cbor::as_text).unwrap_or_default(),
                "CredentialID": id,
                "Name": sent["Name"],
                "Flags": 0
            }));
            state.registered = Some(sent);
            ok()
        }
        ("GET" | "POST", "/core/v4/settings/2fa/register") => refused(),
        (method, path) => {
            let Some(rest) = path.strip_prefix("/core/v4/settings/2fa/") else {
                return not_found();
            };
            let Some((encoded, action)) = rest.split_once('/') else {
                return not_found();
            };
            let mut state = backend.lock();
            let Some(index) = key_index(&state, encoded) else {
                return not_found();
            };
            match (method, action) {
                ("PUT", "rename") => {
                    state.keys[index]["Name"] = request.json()["Name"].clone();
                    ok()
                }
                ("POST", "remove") if state.scoped => {
                    state.keys.remove(index);
                    ok()
                }
                ("POST", "remove") => refused(),
                _ => not_found(),
            }
        }
    }
}

async fn signed_in(backend: Shared) -> Account {
    let srp = Arc::new(FakeSrp::with_password(PASSWORD));
    account(move |request: Request| respond(&backend, &srp, &request)).await
}

#[tokio::test(flavor = "multi_thread")]
async fn security_key_registration_matches_web() {
    let backend = Shared::default();
    let account = signed_in(backend.clone()).await;
    account
        .password_scope()
        .username("tester")
        .password(PASSWORD)
        .execute()
        .await
        .unwrap();

    let keys = account.security_keys();
    let challenge = keys.challenge(true).await.unwrap();
    assert_eq!(
        challenge.attestation_formats(),
        ["packed", "fido-u2f", "none"]
    );
    assert_eq!(
        challenge
            .registration_options()
            .excluded_credentials()
            .collect::<Vec<_>>(),
        vec![[9u8, 9, 9, 9].as_slice()]
    );

    let created = Fido::with_transport(FakeKey::new(Kind::Ctap21))
        .register(challenge.registration_request())
        .unwrap()
        .finish()
        .await
        .unwrap();
    assert_eq!(created.cred_props(), Some(false));

    let registered = keys
        .register(&challenge, &created, "YubiKey")
        .await
        .unwrap();
    assert_eq!(registered.name(), "YubiKey");
    assert_eq!(registered.credential_id(), created.credential_id());
    assert_eq!(registered.attestation_format(), "none");

    let state = backend.lock();
    assert_eq!(state.locks, 1, "sensitive settings not relocked");
    let sent = state.registered.clone().unwrap();
    assert_eq!(sent["RegistrationOptions"], registration_options());
    assert_eq!(sent["Transports"], json!(["usb"]));
    assert_eq!(sent["Name"], "YubiKey");

    let client_data: Value = serde_json::from_slice(
        &rikka_proton_api::base64::decode(sent["ClientData"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(client_data["type"], "webauthn.create");
    assert_eq!(client_data["origin"], "https://account.proton.me");
    assert_eq!(
        client_data["challenge"],
        BASE64_URL_SAFE_NO_PAD.encode((100u8..132).collect::<Vec<u8>>())
    );

    let object = Cbor::decode(
        &rikka_proton_api::base64::decode(sent["AttestationObject"].as_str().unwrap()).unwrap(),
    )
    .unwrap();
    let auth_data = object.get("authData").and_then(Cbor::as_bytes).unwrap();
    assert_eq!(auth_data[..32], rikka_proton_pgp::sha256(PROTON_RP));
}

#[tokio::test(flavor = "multi_thread")]
async fn registered_keys_list_rename_remove() {
    let backend = Shared::default();
    let first = vec![0xfa, 0xfb, 0xff, 0x3e];
    let second = vec![0x01, 0x02];
    backend.lock().keys = vec![
        json!({"AttestationFormat": "packed", "CredentialID": first, "Name": "Old", "Flags": 1}),
        json!({"AttestationFormat": "fido-u2f", "CredentialID": second, "Name": "Spare", "Flags": 0}),
    ];
    let account = signed_in(backend.clone()).await;
    let keys = account.security_keys();

    let listed = keys.list().await.unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed[0].is_compromised());
    assert!(!listed[1].is_compromised());

    keys.rename(&first, "Renamed").await.unwrap();
    assert_eq!(keys.list().await.unwrap()[0].name(), "Renamed");

    assert!(
        keys.remove([&first]).await.is_err(),
        "removal requires password scope"
    );
    account
        .password_scope()
        .username("tester")
        .password(PASSWORD)
        .execute()
        .await
        .unwrap();
    keys.remove([&first, &second]).await.unwrap();

    assert!(keys.list().await.unwrap().is_empty());
    assert_eq!(backend.lock().locks, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_security_key_name_over_128() {
    let backend = Shared::default();
    let first = vec![0xfa, 0xfb];
    backend.lock().keys = vec![
        json!({"AttestationFormat": "packed", "CredentialID": first, "Name": "Old", "Flags": 3}),
    ];
    let account = signed_in(backend.clone()).await;
    let keys = account.security_keys();

    let error = keys.rename(&first, "é".repeat(129)).await.unwrap_err();
    assert!(matches!(
        error,
        Error::SecurityKeyNameTooLong {
            length: 129,
            limit: 128
        }
    ));
    assert_eq!(keys.list().await.unwrap()[0].name(), "Old");

    keys.rename(&first, "é".repeat(128)).await.unwrap();
    assert_eq!(keys.list().await.unwrap()[0].name(), "é".repeat(128));
}

#[tokio::test(flavor = "multi_thread")]
async fn no_attestation_gives_anonymous_credential() {
    let cases = [
        (Kind::Ctap21, "none", "none", [0u8; 16], 0),
        (Kind::Ctap21, "direct", "none", [0x11; 16], 0),
        (Kind::U2f, "none", "none", [0; 16], 0),
        (Kind::U2f, "direct", "fido-u2f", [0; 16], 2),
    ];

    for (kind, request, format, aaguid, statement_fields) in cases {
        let mut options = registration_options();
        options["publicKey"]["attestation"] = json!(request);
        let challenge: SecurityKeyChallenge =
            serde_json::from_value(json!({"RegistrationOptions": options})).unwrap();

        let created = Fido::with_transport(FakeKey::new(kind))
            .register(challenge.registration_request())
            .unwrap()
            .finish()
            .await
            .unwrap();

        let object = Cbor::decode(created.attestation_object()).unwrap();
        assert_eq!(
            object.get("fmt").and_then(Cbor::as_text),
            Some(format),
            "{request}"
        );
        assert_eq!(
            object
                .get("attStmt")
                .and_then(Cbor::as_map)
                .map(<[(Cbor, Cbor)]>::len),
            Some(statement_fields),
            "{request}"
        );
        assert_eq!(&created.authenticator_data()[37..53], &aaguid, "{request}");
    }
}
