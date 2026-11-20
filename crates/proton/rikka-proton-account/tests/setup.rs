use crate::support::fixture::{EMAIL, PASSWORD, Server, address, ok, passphrase, user_key};
use rikka_proton_account::{AddressId, Error, SignedKeyList, SignedKeyListCheck};
use rikka_proton_pgp::{LockedKey, Pgp};
use serde_json::{Value, json};

fn setting_up() -> Server {
    Server::new(
        Vec::new(),
        vec![address("address-value", EMAIL, Vec::new())],
    )
}

fn stored(server: &Server, sent: &Value) {
    let sent_key = &sent["AddressKeys"][0];
    let held_key = json!({
        "ID": "address-key",
        "Version": 3,
        "Flags": 3,
        "PrivateKey": sent_key["PrivateKey"],
        "Token": sent_key["Token"],
        "Signature": sent_key["Signature"],
        "Primary": 1,
        "Active": 1
    });
    let mut held_address = address("address-value", EMAIL, vec![held_key]);
    held_address["SignedKeyList"] = sent_key["SignedKeyList"].clone();

    server.with(|state| {
        state.user_keys = vec![user_key(
            "user-key",
            sent["PrimaryKey"].as_str().unwrap_or_default(),
            true,
        )];
        state.salts = vec![json!({"ID": "user-key", "KeySalt": sent["KeySalt"]})];
        state.addresses = vec![held_address];
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn setup_creates_user_and_address_keys() {
    let server = setting_up();
    let account = server
        .account(|server, ask| match ask.route() {
            ("POST", "/core/v4/keys/setup") => {
                stored(server, &ask.json());
                Some(ok())
            }
            _ => None,
        })
        .await;

    let keys = account
        .setup_keys()
        .password(PASSWORD)
        .execute()
        .await
        .unwrap();

    assert_eq!(keys.unlocked_key_count(), 2);
    assert!(
        keys.address_keys(&AddressId::from("address-value"))
            .is_some()
    );

    let sent = server.last_body("POST", "/core/v4/keys/setup").unwrap();
    assert_eq!(sent["Auth"]["Version"], 4);
    assert_eq!(sent["Auth"]["ModulusID"], "modulus-id");
    assert!(sent["Auth"]["Verifier"].is_string());
    assert!(sent.get("EncryptedSecret").is_none());

    let salt = sent["KeySalt"].as_str().unwrap();
    LockedKey::import(sent["PrimaryKey"].as_str().unwrap_or_default())
        .unwrap()
        .unlock(passphrase(PASSWORD, salt))
        .unwrap();

    let sent_key = &sent["AddressKeys"][0];
    assert_eq!(sent_key["AddressID"], "address-value");
    assert_eq!(sent_key["Primary"], 1);

    let list: SignedKeyList = serde_json::from_value(sent_key["SignedKeyList"].clone()).unwrap();
    let public = LockedKey::import(sent_key["PrivateKey"].as_str().unwrap_or_default())
        .unwrap()
        .public_key();
    assert_eq!(
        list.check(&Pgp::new(), std::slice::from_ref(&public)),
        SignedKeyListCheck::Verified
    );
    assert!(
        list.data()
            .unwrap_or_default()
            .starts_with(r#"[{"Primary":1,"Flags":3,"Fingerprint":""#)
    );

    let items = list.key_list().unwrap().into_items();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].fingerprint(), public.fingerprint().as_str());
    assert_eq!(
        items[0].sha256_fingerprints().len(),
        public.fingerprints().len()
    );
    assert!(
        items[0]
            .sha256_fingerprints()
            .iter()
            .all(|fingerprint| fingerprint.len() == 64)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn setup_skips_account_with_keys() {
    let server = Server::new(
        vec![user_key("user-key", "armored", true)],
        vec![address("address-value", EMAIL, Vec::new())],
    );
    let account = server.account(|_, _| None).await;

    let error = account
        .setup_keys()
        .password(PASSWORD)
        .execute()
        .await
        .unwrap_err();

    assert!(matches!(error, Error::KeysAlreadySetUp));
    assert!(server.last_body("POST", "/core/v4/keys/setup").is_none());
}
