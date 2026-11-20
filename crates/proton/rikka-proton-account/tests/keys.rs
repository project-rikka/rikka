use rikka_proton_account::{Account, AddressId, Error, KeyId};
use rikka_proton_pgp::{Pgp, PrivateKey, Srp};
use rikka_test_proton::{Request, lock_key, new_key, not_found, response};
use serde_json::{Value, json};

const PASSWORD: &str = "the account password";
const SALT: &str = "jQ0LTS8bBqQQZZL4MzmtQQ==";
const TOKEN: &str = "3d2b0d1f4c5a6e7b8c9d0e1f2a3b4c5d6e7f80917a2b3c4d5e6f708192a3b4c5";

struct KeyFixture {
    user_key: String,
    address_keys: Vec<Value>,
}

fn passphrase() -> Vec<u8> {
    let salt = rikka_proton_api::base64::decode(SALT).unwrap();

    Srp::new()
        .mailbox_passphrase(PASSWORD, salt)
        .unwrap()
        .as_bytes()
        .to_vec()
}

fn address_key(id: &str, private_key: String, token: Option<(String, String)>) -> Value {
    let mut key = json!({
        "ID": id,
        "Version": 3,
        "Flags": 3,
        "PrivateKey": private_key,
        "Primary": 1,
        "Active": 1
    });

    if let Some((token, signature)) = token {
        key["Token"] = Value::String(token);
        key["Signature"] = Value::String(signature);
    }

    key
}

fn token_for(pgp: &Pgp, user_key: &PrivateKey, signer: &PrivateKey) -> (String, String) {
    let token = pgp
        .encrypt()
        .to(user_key.public_key())
        .armored_message(TOKEN)
        .unwrap();
    let signature = pgp
        .sign()
        .with_key(signer.clone())
        .armored_detached(TOKEN)
        .unwrap();

    (token, signature)
}

async fn fixture(key_fixture: KeyFixture) -> Account {
    let user_key = key_fixture.user_key;
    let address_keys = key_fixture.address_keys;

    crate::support::account(move |request: Request| {
        let user_key = user_key.clone();
        let address_keys = address_keys.clone();

        match request.route() {
            ("GET", "/core/v4/users") => response(
                200,
                json!({
                    "Code": 1000,
                    "User": {
                        "ID": "user-value",
                        "Name": "tester",
                        "Keys": [{
                            "ID": "user-key",
                            "Version": 3,
                            "PrivateKey": user_key,
                            "Primary": 1,
                            "Active": 1
                        }]
                    }
                })
                .to_string(),
            ),
            ("GET", "/core/v4/addresses") => response(
                200,
                json!({
                    "Code": 1000,
                    "Total": 1,
                    "Addresses": [{
                        "ID": "address-value",
                        "Email": "tester@proton.test",
                        "Send": 1,
                        "Receive": 1,
                        "Status": 1,
                        "Type": 1,
                        "Order": 1,
                        "HasKeys": 1,
                        "Keys": address_keys
                    }]
                })
                .to_string(),
            ),
            ("GET", "/core/v4/keys/salts") => response(
                200,
                json!({"Code": 1000, "KeySalts": [{"ID": "user-key", "KeySalt": SALT}]})
                    .to_string(),
            ),
            _ => not_found(),
        }
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn migrated_address_key_opens_with_token() {
    let pgp = Pgp::new();
    let user = new_key(&pgp, "tester@proton.test");
    let address = new_key(&pgp, "tester@proton.test");
    let (token, signature) = token_for(&pgp, &user, &user);

    let account = fixture(KeyFixture {
        user_key: lock_key(&user, passphrase()),
        address_keys: vec![address_key(
            "address-key",
            lock_key(&address, TOKEN),
            Some((token, signature)),
        )],
    })
    .await;

    let keys = account.unlock(PASSWORD).await.unwrap();
    let address_id = AddressId::from("address-value");

    assert_eq!(keys.user_keys().private_keys().len(), 1);
    assert!(keys.user_key(&KeyId::from("user-key")).is_some());
    assert!(keys.primary_user_key().is_some());
    assert_eq!(
        keys.address_keys(&address_id).unwrap().private_keys().len(),
        1
    );
    assert_eq!(
        keys.address_key(&KeyId::from("address-key"))
            .unwrap()
            .fingerprint(),
        address.fingerprint()
    );
    assert!(keys.public_key(&KeyId::from("user-key")).is_some());
    assert_eq!(
        keys.primary_address().ok().map(|address| address.email()),
        Some("tester@proton.test")
    );
    assert_eq!(
        keys.address_by_email("TESTER@proton.test").map(|a| a.id()),
        Some(&address_id)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_token_signed_by_unknown_key() {
    let pgp = Pgp::new();
    let user = new_key(&pgp, "tester@proton.test");
    let stranger = new_key(&pgp, "stranger@proton.test");
    let address = new_key(&pgp, "tester@proton.test");
    let (token, signature) = token_for(&pgp, &user, &stranger);

    let account = fixture(KeyFixture {
        user_key: lock_key(&user, passphrase()),
        address_keys: vec![address_key(
            "address-key",
            lock_key(&address, TOKEN),
            Some((token, signature)),
        )],
    })
    .await;

    let keys = account.unlock(PASSWORD).await.unwrap();

    assert_eq!(keys.user_keys().private_keys().len(), 1);
    assert!(
        keys.address_keys(&AddressId::from("address-value"))
            .is_none()
    );
    assert!(keys.address_key(&KeyId::from("address-key")).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn legacy_address_key_opens_with_passphrase() {
    let pgp = Pgp::new();
    let user = new_key(&pgp, "tester@proton.test");
    let address = new_key(&pgp, "tester@proton.test");

    let account = fixture(KeyFixture {
        user_key: lock_key(&user, passphrase()),
        address_keys: vec![address_key(
            "address-key",
            lock_key(&address, passphrase()),
            None,
        )],
    })
    .await;

    let keys = account.unlock(PASSWORD).await.unwrap();

    assert!(
        keys.address_keys(&AddressId::from("address-value"))
            .is_some()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_password_opening_no_key() {
    let pgp = Pgp::new();
    let user = new_key(&pgp, "tester@proton.test");

    let account = fixture(KeyFixture {
        user_key: lock_key(&user, passphrase()),
        address_keys: Vec::new(),
    })
    .await;

    let error = account.unlock("another password").await.unwrap_err();

    assert!(matches!(error, Error::NoKeyUnlocked));
}
