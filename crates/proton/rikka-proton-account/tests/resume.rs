use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use rikka_proton_account::{Account, AddressId, Profile};
use rikka_proton_cache::Cache;
use rikka_proton_pgp::{Pgp, PrivateKey, SecretBytes, Srp, SymmetricKey};
use rikka_test_proton::{Request, ResponseFuture, not_found, response};
use serde_json::{Value, json};

const PASSWORD: &str = "the account password";
const SALT: &str = "jQ0LTS8bBqQQZZL4MzmtQQ==";
const TOKEN: &str = "3d2b0d1f4c5a6e7b8c9d0e1f2a3b4c5d6e7f80917a2b3c4d5e6f708192a3b4c5";

struct KeyFixture {
    user_key: String,
    address_key: Value,
}

#[derive(Clone)]
struct Server {
    key_fixture: Arc<Mutex<KeyFixture>>,
    used_space: Arc<AtomicUsize>,
    request: Arc<AtomicUsize>,
}

impl Server {
    fn request(&self) -> usize {
        self.request.load(Ordering::SeqCst)
    }

    fn respond(&self, request: &Request) -> ResponseFuture {
        self.request.fetch_add(1, Ordering::SeqCst);
        let key_fixture = self.key_fixture.lock().unwrap();

        match request.route() {
            ("GET", "/core/v4/users") => response(
                200,
                json!({
                    "Code": 1000,
                    "User": {
                        "ID": "user-value",
                        "Name": "tester",
                        "UsedSpace": self.used_space.load(Ordering::SeqCst),
                        "Keys": [{
                            "ID": "user-key",
                            "Version": 3,
                            "PrivateKey": key_fixture.user_key,
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
                        "Keys": [key_fixture.address_key.clone()]
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
    }
}

fn passphrase() -> Vec<u8> {
    let salt = rikka_proton_api::base64::decode(SALT).unwrap();

    Srp::new()
        .mailbox_passphrase(PASSWORD, salt)
        .unwrap()
        .as_bytes()
        .to_vec()
}

fn new_key(pgp: &Pgp) -> PrivateKey {
    pgp.generate_key()
        .user_id("tester", "tester@proton.test")
        .generate()
        .unwrap()
}

fn key_fixture(pgp: &Pgp) -> KeyFixture {
    let user = new_key(pgp);
    let address = new_key(pgp);
    let token = pgp
        .encrypt()
        .to(user.public_key())
        .armored_message(TOKEN)
        .unwrap();
    let signature = pgp
        .sign()
        .with_key(user.clone())
        .armored_detached(TOKEN)
        .unwrap();

    KeyFixture {
        user_key: armored(&user, passphrase()),
        address_key: json!({
            "ID": "address-key",
            "Version": 3,
            "Flags": 3,
            "PrivateKey": armored(&address, TOKEN),
            "Token": token,
            "Signature": signature,
            "Primary": 1,
            "Active": 1
        }),
    }
}

fn armored(key: &PrivateKey, secret: impl AsRef<[u8]>) -> String {
    key.lock(secret).unwrap().export_armored().unwrap()
}

async fn start_server(pgp: &Pgp) -> (Account, Server) {
    let server = Server {
        key_fixture: Arc::new(Mutex::new(key_fixture(pgp))),
        used_space: Arc::new(AtomicUsize::new(10)),
        request: Arc::new(AtomicUsize::new(0)),
    };
    let handler = server.clone();
    let account = crate::support::account(move |request: Request| handler.respond(&request)).await;

    (account, server)
}

#[tokio::test(flavor = "multi_thread")]
async fn stored_profile_opens_keys_offline() {
    let pgp = Pgp::new();
    let (account, server) = start_server(&pgp).await;
    let profile = account.profile().await.unwrap();
    let stored: Profile = serde_json::from_slice(&serde_json::to_vec(&profile).unwrap()).unwrap();
    let request = server.request();

    let keys = account
        .unlock_profile(stored, Some(SecretBytes::new(passphrase())))
        .await
        .unwrap();

    assert_eq!(server.request(), request);
    assert_eq!(keys.unlocked_key_count(), 2);
    assert!(
        keys.address_keys(&AddressId::from("address-value"))
            .is_some()
    );
    assert_eq!(keys.profile(), profile);
}

#[tokio::test(flavor = "multi_thread")]
async fn stored_profile_rejects_wrong_passphrase() {
    let pgp = Pgp::new();
    let (account, _server) = start_server(&pgp).await;
    let profile = account.profile().await.unwrap();

    let refused = account
        .unlock_profile(profile, Some(SecretBytes::new(b"another".to_vec())))
        .await;

    assert!(matches!(
        refused,
        Err(rikka_proton_account::Error::NoKeyUnlocked)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn unlocked_profile_is_cached() {
    let pgp = Pgp::new();
    let (account, server) = start_server(&pgp).await;
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let account = account.with_cache(cache.clone());
    assert_eq!(account.cached_profile(), None);

    let keys = account.unlock(PASSWORD).await.unwrap();
    cache.flush().unwrap();
    let request = server.request();

    let later = account.clone();
    let stored = later.cached_profile().unwrap();
    assert_eq!(stored, keys.profile());
    let reopened = later
        .unlock_profile(stored, Some(SecretBytes::new(passphrase())))
        .await
        .unwrap();
    assert_eq!(reopened.unlocked_key_count(), 2);
    assert_eq!(server.request(), request);
}
