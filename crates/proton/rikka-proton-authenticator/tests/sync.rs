use base64::prelude::{BASE64_STANDARD, Engine as _};
use jiff::Timestamp;
use rikka_proton_account::{Account as ProtonAccount, AddressId, UserKeys};
use rikka_proton_api::Api;
use rikka_proton_authenticator::{
    AuthenticatorClient, AuthenticatorKey, Entry, EntryState, Error, LocalEntry, Placement,
};
use rikka_proton_cache::Cache;
use rikka_proton_pgp::{Clock, KeyRing, Pgp, PrivateKey, Srp, SymmetricKey};
use rikka_test_proton::{Record, Request, api, not_found, response};
use serde_json::{Value, json};

const NOW: i64 = 1_700_000_000;

fn timestamp(second: i64) -> Timestamp {
    Timestamp::from_second(second).unwrap()
}
const PASSWORD: &str = "the account password";
const SALT: &str = "jQ0LTS8bBqQQZZL4MzmtQQ==";

struct Account {
    pgp: Pgp,
    keys: UserKeys,
}

impl Account {
    fn new() -> Account {
        let pgp = Pgp::with_clock(Clock::fixed(1_700_000_000));
        let key: PrivateKey = pgp
            .generate_key()
            .user_id("tester", "person@proton.me")
            .generate()
            .unwrap();

        Account {
            pgp,
            keys: UserKeys::new(KeyRing::new().with_private(key.clone()), key),
        }
    }

    fn wrapped(&self, key: &AuthenticatorKey) -> String {
        key.wrap(&self.pgp, &self.keys).unwrap()
    }

    fn client(&self, api: Api) -> AuthenticatorClient {
        AuthenticatorClient::with_pgp(api, self.pgp, self.keys.clone()).unwrap()
    }
}

fn entry(id: &str, issuer: &str) -> Entry {
    Entry::from_uri(
        id,
        &format!("otpauth://totp/person?secret=JBSWY3DPEHPK3PXP&issuer={issuer}"),
    )
    .unwrap()
}

fn keys_response(wrapped: &str) -> String {
    json!({
        "Code": 1000,
        "Keys": {"Keys": [{"KeyID": "key-one", "Key": wrapped, "UserKeyID": "user-one"}]}
    })
    .to_string()
}

fn payload(id: &str, revision: u32, content: &str) -> Value {
    json!({
        "EntryID": id,
        "AuthenticatorKeyID": "key-one",
        "Revision": revision,
        "ContentFormatVersion": 1,
        "Content": content,
        "Flags": 0,
        "CreateTime": NOW,
        "ModifyTime": NOW
    })
}

fn page(entries: Vec<Value>, last_id: Option<&str>) -> String {
    json!({
        "Code": 1000,
        "Entries": {"Entries": entries, "Total": 1, "LastID": last_id}
    })
    .to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn entries_page_until_last_id() {
    let account = Account::new();
    let key = AuthenticatorKey::generate();
    let wrapped = account.wrapped(&key);
    let first = key.seal(&entry("one", "Proton")).unwrap();
    let second = key.seal(&entry("two", "Other")).unwrap();
    let record = Record::default();
    let recorder = record.clone();

    let api = api(move |request: Request| {
        recorder.push(&request);
        match request.route() {
            ("GET", "/authenticator/v1/key") => response(200, keys_response(&wrapped)),
            ("GET", "/authenticator/v1/entry") if request.query.contains("Since=remote-one") => {
                response(200, page(vec![payload("remote-two", 4, &second)], None))
            }
            ("GET", "/authenticator/v1/entry") => response(
                200,
                page(vec![payload("remote-one", 1, &first)], Some("remote-one")),
            ),
            _ => not_found(),
        }
    })
    .await;

    let client = account.client(api);
    let keys = client.keys().await.unwrap();
    let entries = client.entries(&keys).await.unwrap();

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].remote_id, "remote-one");
    assert_eq!(entries[0].revision, 1);
    assert_eq!(entries[0].entry, entry("one", "Proton"));
    assert_eq!(entries[1].remote_id, "remote-two");
    assert_eq!(entries[1].revision, 4);
    assert_eq!(entries[1].entry.issuer(), Some("Other"));
    assert_eq!(record.matching("GET", "/authenticator/v1/entry").len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn cached_keys_and_entries_open_offline() {
    let account = Account::new();
    let key = AuthenticatorKey::generate();
    let wrapped = account.wrapped(&key);
    let first = key.seal(&entry("one", "Proton")).unwrap();
    let second = key.seal(&entry("two", "Other")).unwrap();

    let api = api(move |request: Request| match request.route() {
        ("GET", "/authenticator/v1/key") => response(200, keys_response(&wrapped)),
        ("GET", "/authenticator/v1/entry") => response(
            200,
            page(
                vec![
                    payload("remote-one", 1, &first),
                    payload("remote-two", 2, &second),
                ],
                None,
            ),
        ),
        ("DELETE", "/authenticator/v1/entry/bulk") => response(200, r#"{"Code":1000}"#),
        _ => not_found(),
    })
    .await;
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let client = account.client(api.clone()).with_cache(cache.clone());
    let keys = client.keys().await.unwrap();
    client.entries(&keys).await.unwrap();
    client.delete_entry("remote-one").await.unwrap();
    cache.flush().unwrap();

    let offline = account.client(api).with_cache(cache);
    let cached_keys = offline.cached_keys().await.unwrap().unwrap();
    let cached = offline.cached_entries(&cached_keys).await.unwrap();
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].remote_id, "remote-two");
    assert_eq!(cached[0].entry, entry("two", "Other"));
}

#[tokio::test(flavor = "multi_thread")]
async fn created_entry_sealed_under_named_key() {
    let account = Account::new();
    let key = AuthenticatorKey::generate();
    let wrapped = account.wrapped(&key);
    let record = Record::default();
    let recorder = record.clone();

    let api = api(move |request: Request| {
        recorder.push(&request);
        match request.route() {
            ("GET", "/authenticator/v1/key") => response(200, keys_response(&wrapped)),
            ("POST", "/authenticator/v1/entry") => {
                let content = request.json()["Content"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                response(
                    200,
                    json!({"Code": 1000, "Entry": payload("remote-one", 1, &content)}).to_string(),
                )
            }
            _ => not_found(),
        }
    })
    .await;

    let client = account.client(api);
    let keys = client.keys().await.unwrap();
    let created = client
        .create_entry(&keys[0], &entry("one", "Proton"))
        .await
        .unwrap();

    assert_eq!(created.remote_id, "remote-one");
    assert_eq!(created.entry, entry("one", "Proton"));

    let request = record.matching("POST", "/authenticator/v1/entry");
    assert_eq!(request[0].json()["AuthenticatorKeyID"], json!("key-one"));
    assert_eq!(request[0].json()["ContentFormatVersion"], json!(1));

    let sent = request[0].json()["Content"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert_eq!(key.open(&sent).unwrap(), entry("one", "Proton"));
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_pushes_local_and_pulls_remote() {
    let account = Account::new();
    let key = AuthenticatorKey::generate();
    let wrapped = account.wrapped(&key);
    let sealed_entry = key.seal(&entry("held", "Proton")).unwrap();
    let foreign_content = AuthenticatorKey::generate()
        .seal(&entry("sealed", "Other"))
        .unwrap();
    let record = Record::default();
    let recorder = record.clone();

    let api = api(move |request: Request| {
        recorder.push(&request);
        match request.route() {
            ("GET", "/authenticator/v1/key") => response(200, keys_response(&wrapped)),
            ("GET", "/authenticator/v1/entry") => response(
                200,
                page(
                    vec![
                        payload("remote-held", 1, &sealed_entry),
                        payload("remote-sealed", 1, &foreign_content),
                    ],
                    None,
                ),
            ),
            ("POST", "/authenticator/v1/entry/bulk") => {
                let content = request.json()["Entries"][0]["Content"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                response(
                    200,
                    json!({"Code": 1000, "Entries": [payload("remote-new", 1, &content)]})
                        .to_string(),
                )
            }
            ("DELETE", "/authenticator/v1/entry/bulk") => response(200, r#"{"Code":1000}"#),
            ("PUT", "/authenticator/v1/entry/order") => response(
                200,
                json!({"Code": 1000, "Result": {"ModifyTime": NOW + 5}}).to_string(),
            ),
            _ => not_found(),
        }
    })
    .await;

    let local = vec![
        LocalEntry::new(
            entry("held", "Proton"),
            EntryState::Synced,
            0,
            timestamp(NOW),
        ),
        LocalEntry::new(
            entry("fresh", "Fresh"),
            EntryState::PendingSync,
            1,
            timestamp(NOW),
        ),
        LocalEntry::new(entry("gone", "Gone"), EntryState::Synced, 2, timestamp(NOW)),
    ];

    let outcome = account.client(api).sync(&local).await.unwrap();

    assert_eq!(outcome.undecryptable, 1);
    assert_eq!(outcome.removed, vec!["gone".to_owned()]);
    assert_eq!(outcome.entries.len(), 1);
    assert_eq!(outcome.entries[0].remote_id, "remote-new");
    assert_eq!(outcome.entries[0].entry, entry("fresh", "Fresh"));
    assert_eq!(outcome.entries[0].position, Some(1));
    assert_eq!(outcome.entries[0].modified_at, timestamp(NOW));
    assert_eq!(
        outcome.order,
        vec![Placement {
            id: "held".to_owned(),
            remote_id: "remote-held".to_owned(),
            position: 0,
            modified_at: timestamp(NOW + 5),
        }]
    );
    assert!(
        record
            .matching("DELETE", "/authenticator/v1/entry/bulk")
            .is_empty()
    );

    let ordered = record.matching("PUT", "/authenticator/v1/entry/order");
    assert_eq!(
        ordered[0].json(),
        json!({"StartingPosition": 0, "Entries": ["remote-held"]})
    );

    let created = record.matching("POST", "/authenticator/v1/entry/bulk");
    assert_eq!(created.len(), 1);
    assert_eq!(
        created[0].json()["Entries"][0]["AuthenticatorKeyID"],
        json!("key-one")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_propagates_deletions() {
    let account = Account::new();
    let key = AuthenticatorKey::generate();
    let wrapped = account.wrapped(&key);
    let deleted_content = key.seal(&entry("deleted_content", "Proton")).unwrap();
    let record = Record::default();
    let recorder = record.clone();

    let api = api(move |request: Request| {
        recorder.push(&request);
        match request.route() {
            ("GET", "/authenticator/v1/key") => response(200, keys_response(&wrapped)),
            ("GET", "/authenticator/v1/entry") => response(
                200,
                page(
                    vec![payload("remote-deleted_content", 2, &deleted_content)],
                    None,
                ),
            ),
            ("DELETE", "/authenticator/v1/entry/bulk") => response(200, r#"{"Code":1000}"#),
            ("PUT", "/authenticator/v1/entry/order") => response(
                200,
                json!({"Code": 1000, "Result": {"ModifyTime": NOW}}).to_string(),
            ),
            _ => not_found(),
        }
    })
    .await;

    let local = vec![LocalEntry::new(
        entry("deleted_content", "Proton"),
        EntryState::PendingDelete,
        0,
        timestamp(NOW),
    )];

    let outcome = account.client(api).sync(&local).await.unwrap();

    assert!(outcome.entries.is_empty());
    assert_eq!(outcome.removed, vec!["deleted_content".to_owned()]);
    assert_eq!(
        record.matching("DELETE", "/authenticator/v1/entry/bulk")[0].json()["EntryIDs"],
        json!(["remote-deleted_content"])
    );
}

async fn client_with_key_signed_by_address_key() -> AuthenticatorClient {
    let pgp = Pgp::new();
    let new_key = |name: &str| {
        pgp.generate_key()
            .user_id(name, "person@proton.me")
            .generate()
            .unwrap()
    };
    let user_key = new_key("tester");
    let address_key = new_key("address");

    let salt = BASE64_STANDARD.decode(SALT).unwrap();
    let passphrase = Srp::new().mailbox_passphrase(PASSWORD, salt).unwrap();
    let lock_key = |key: &PrivateKey| {
        key.lock(passphrase.as_bytes())
            .unwrap()
            .export_armored()
            .unwrap()
    };
    let (locked_user, locked_address) = (lock_key(&user_key), lock_key(&address_key));

    let key = AuthenticatorKey::generate();
    let message = pgp
        .encrypt()
        .to(user_key.public_key())
        .signed_by(address_key.clone())
        .message(key.as_bytes())
        .unwrap();
    let wrapped = BASE64_STANDARD.encode(message);

    let api = api(move |request: Request| match request.route() {
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
                        "PrivateKey": locked_user,
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
                    "Email": "person@proton.me",
                    "Send": 1,
                    "Receive": 1,
                    "Status": 1,
                    "Type": 1,
                    "Order": 1,
                    "HasKeys": 1,
                    "Keys": [{
                        "ID": "address-key",
                        "Version": 3,
                        "Flags": 3,
                        "PrivateKey": locked_address,
                        "Primary": 1,
                        "Active": 1
                    }]
                }]
            })
            .to_string(),
        ),
        ("GET", "/core/v4/keys/salts") => response(
            200,
            json!({"Code": 1000, "KeySalts": [{"ID": "user-key", "KeySalt": SALT}]}).to_string(),
        ),
        ("GET", "/authenticator/v1/key") => response(200, keys_response(&wrapped)),
        _ => not_found(),
    })
    .await;

    let account = ProtonAccount::new(api);
    let account_keys = std::sync::Arc::new(account.unlock(PASSWORD).await.unwrap());
    assert!(
        account_keys
            .address_keys(&AddressId::from("address-value"))
            .is_some(),
        "address key did not unlock"
    );
    AuthenticatorClient::for_account(&account, &account_keys).unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_key_signed_by_address_key() {
    let client = client_with_key_signed_by_address_key().await;

    let error = client.keys().await.unwrap_err();

    assert!(matches!(error, Error::NoUsableKey));
}
