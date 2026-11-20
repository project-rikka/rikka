use crate::support::fixture::{
    EMAIL, PASSWORD, SALT, Server, address, address_key, migrated, new_key, ok, passphrase,
    user_key,
};
use rikka_proton_account::{AddressId, Error, SignedKeyList, SignedKeyListCheck};
use rikka_proton_pgp::{KeyKind, LockedKey, Pgp, unarmor};
use rikka_test_proton::{lock_key, response};
use serde_json::{Value, json};

#[tokio::test(flavor = "multi_thread")]
async fn post_quantum_key_cosigns_key_list() {
    let pgp = Pgp::new();
    let original_keys = migrated(&pgp);
    let account = original_keys
        .server
        .account(|server, request| match request.route() {
            ("POST", "/core/v4/keys/address") => {
                let sent = request.json();
                let stored = json!({
                    "ID": "v6-key",
                    "Version": 3,
                    "Flags": 3,
                    "PrivateKey": sent["PrivateKey"],
                    "Token": sent["Token"],
                    "Signature": sent["Signature"],
                    "Primary": 1,
                    "Active": 1
                });
                server.with(|state| {
                    state.addresses[0]["Keys"]
                        .as_array_mut()
                        .unwrap()
                        .push(stored.clone());
                });
                Some(response(
                    200,
                    json!({"Code": 1000, "Key": stored}).to_string(),
                ))
            }
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    account
        .create_address_key(&keys, &AddressId::from("address-value"))
        .kind(KeyKind::PostQuantum)
        .execute()
        .await
        .unwrap();

    let sent = original_keys
        .server
        .last_body("POST", "/core/v4/keys/address")
        .unwrap();
    assert_eq!(sent["Primary"], 1);
    let v6 = LockedKey::import(sent["PrivateKey"].as_str().unwrap_or_default())
        .unwrap()
        .public_key();
    assert_eq!(v6.version(), 6);

    let list: SignedKeyList = serde_json::from_value(sent["SignedKeyList"].clone()).unwrap();
    let v4 = original_keys.address_key.public_key();
    assert_eq!(
        list.check(&pgp, &[v4.clone(), v6.clone()]),
        SignedKeyListCheck::Verified
    );
    let items = list.key_list().unwrap().into_items();
    assert!(items[0].matches_key(&v4) && items[0].is_primary());
    assert!(items[1].matches_key(&v6) && items[1].is_primary());
    assert!(list.verify(&pgp, std::slice::from_ref(&v6)).is_valid());
    assert!(list.verify(&pgp, std::slice::from_ref(&v4)).is_valid());

    let reopened = account.unlock(PASSWORD).await.unwrap();
    let ring = reopened
        .address_keys(&AddressId::from("address-value"))
        .unwrap();
    assert_eq!(ring.private_keys().len(), 2);
    assert!(ring.private_keys().iter().any(|key| key.version() == 6));
}

#[tokio::test(flavor = "multi_thread")]
async fn post_quantum_key_requires_token_keys() {
    let pgp = Pgp::new();
    let user = new_key(&pgp, "not_for_email_use@domain.tld");
    let original_keys = new_key(&pgp, EMAIL);
    let secret = passphrase(PASSWORD, SALT);
    let server = Server::new(
        vec![user_key("user-key", &lock_key(&user, &secret), true)],
        vec![address(
            "address-value",
            EMAIL,
            vec![address_key(
                "address-key",
                &lock_key(&original_keys, &secret),
                None,
                true,
            )],
        )],
    );
    let account = server.account(|_, _| None).await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    let error = account
        .create_address_key(&keys, &AddressId::from("address-value"))
        .kind(KeyKind::PostQuantum)
        .execute()
        .await
        .unwrap_err();

    assert!(matches!(error, Error::PostQuantumNeedsTokens));
}

#[tokio::test(flavor = "multi_thread")]
async fn post_quantum_user_key_reseals_tokens() {
    let pgp = Pgp::new();
    let original_keys = migrated(&pgp);
    let account = original_keys
        .server
        .account(|server, request| match request.route() {
            ("POST", "/core/v4/keys/user") => {
                let sent = request.json();
                server.with(|state| {
                    state.user_keys[0]["Primary"] = json!(0);
                    state.user_keys.push(json!({
                        "ID": "pq-user-key",
                        "Version": 3,
                        "PrivateKey": sent["PrivateKey"],
                        "Primary": 1,
                        "Active": 1
                    }));
                    state
                        .salts
                        .push(json!({"ID": "pq-user-key", "KeySalt": SALT}));
                });
                Some(ok())
            }
            ("PUT", "/core/v4/keys/tokens") => Some(ok()),
            _ => None,
        })
        .await;
    let keys = account.unlock(PASSWORD).await.unwrap();

    let reopened = account
        .create_user_key(&keys)
        .kind(KeyKind::PostQuantum)
        .execute()
        .await
        .unwrap();
    assert_eq!(reopened.user_keys().private_keys().len(), 2);

    let created = original_keys
        .server
        .last_body("POST", "/core/v4/keys/user")
        .unwrap();
    assert_eq!(created["Primary"], 1);
    let new_key = LockedKey::import(created["PrivateKey"].as_str().unwrap_or_default())
        .unwrap()
        .unlock(passphrase(PASSWORD, SALT))
        .unwrap();
    assert_eq!(new_key.version(), 6);

    let replaced = original_keys
        .server
        .last_body("PUT", "/core/v4/keys/tokens")
        .unwrap();
    let token = &replaced["AddressKeyTokens"][0];
    assert_eq!(token["AddressKeyID"], "address-key");

    let key_packets =
        rikka_proton_api::base64::decode(token["KeyPacket"].as_str().unwrap_or_default()).unwrap();
    let resealed = pgp
        .decrypt()
        .with_key(new_key.clone())
        .session_key(&key_packets)
        .unwrap();
    let original = pgp
        .decrypt()
        .with_key(original_keys.user_key.clone())
        .session_key(unarmor(&original_keys.token.sealed).unwrap())
        .unwrap();
    assert_eq!(resealed.as_bytes(), original.as_bytes());
    assert!(
        pgp.verify()
            .with_key(new_key.public_key())
            .detached(
                original_keys.token.secret.as_bytes(),
                Value::as_str(&token["Signature"]).unwrap_or_default()
            )
            .is_valid()
    );
}
