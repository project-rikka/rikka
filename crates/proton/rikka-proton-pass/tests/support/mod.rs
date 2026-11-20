use std::sync::Arc;

use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_account::{Account, UserKeys};
use rikka_proton_pass::{
    EncryptionKey, EncryptionTag, Item, ItemContent, ItemId, ItemLogin, Metadata, PassClient,
    PassItem, ShareId, Vault,
};
use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey, Srp};
use serde_json::{Value, json};

use rikka_test_proton::{Record, Request, ResponseFuture, api, lock_key, not_found, response};

pub(crate) const PASSWORD: &str = "the account password";
pub(crate) const SALT: &str = "jQ0LTS8bBqQQZZL4MzmtQQ==";
pub(crate) const SHARE: &str = "share-value";
pub(crate) const ITEM: &str = "item-value";
pub(crate) const ADDRESS: &str = "address-value";
pub(crate) const EMAIL: &str = "tester@proton.test";

pub(crate) struct Fixture {
    pub(crate) client: PassClient,
    pub(crate) user: PrivateKey,
    pub(crate) address: PrivateKey,
    pub(crate) share_key: EncryptionKey,
    pub(crate) item_key: EncryptionKey,
    pub(crate) record: Record,
}

impl Fixture {
    pub(crate) fn share(&self) -> ShareId {
        ShareId::from(SHARE)
    }

    pub(crate) fn user_keys(&self) -> UserKeys {
        UserKeys::new(
            KeyRing::new().with_private(self.user.clone()),
            self.user.clone(),
        )
    }

    pub(crate) async fn item(&self) -> PassItem {
        self.client
            .item(&self.share(), &ItemId::from(ITEM))
            .await
            .unwrap()
    }
}

pub(crate) fn new_key(email: &str) -> PrivateKey {
    Pgp::new()
        .generate_key()
        .user_id("tester", email)
        .generate()
        .unwrap()
}

pub(crate) fn seal_base64(
    key: &EncryptionKey,
    data: impl AsRef<[u8]>,
    tag: &EncryptionTag,
) -> String {
    BASE64_STANDARD.encode(key.seal(data, tag).unwrap())
}

pub(crate) fn decrypt(key: &EncryptionKey, data: &Value, tag: &EncryptionTag) -> Vec<u8> {
    let decoded = BASE64_STANDARD.decode(data.as_str().unwrap()).unwrap();
    key.open(decoded, tag).unwrap()
}

pub(crate) fn login(name: &str, password: &str) -> Item {
    Item::new(
        Metadata {
            name: name.to_owned(),
            note: String::new(),
            item_uuid: "uuid".to_owned(),
            custom_icon: None,
        },
        ItemContent::Login(ItemLogin {
            item_email: EMAIL.to_owned(),
            password: password.to_owned(),
            ..ItemLogin::default()
        }),
    )
}

pub(crate) fn share_json(id: &str, share_key: &EncryptionKey) -> Value {
    let vault = Vault {
        name: "Personal".to_owned(),
        description: String::new(),
        display: None,
    };

    json!({
        "ShareID": id,
        "VaultID": format!("vault-of-{id}"),
        "AddressID": ADDRESS,
        "TargetType": 1,
        "TargetID": format!("vault-of-{id}"),
        "Permission": 0,
        "Content": seal_base64(share_key, vault.encode(), &EncryptionTag::VaultContent),
        "ContentKeyRotation": 1,
        "ContentFormatVersion": 1,
        "ShareRoleID": "1",
        "Owner": true,
        "Shared": false,
        "CreateTime": 1_700_000_000i64
    })
}

pub(crate) fn share_keys_json(keys: &UserKeys, share_key: &EncryptionKey) -> Value {
    json!({
        "Code": 1000,
        "ShareKeys": {
            "Total": 1,
            "Keys": [{
                "KeyRotation": 1,
                "Key": share_key.wrap(&Pgp::new(), keys).unwrap(),
                "CreateTime": 1
            }]
        }
    })
}

pub(crate) fn revision_json(
    id: &str,
    revision: i64,
    content: &str,
    item_key: &str,
    folder: Option<&str>,
) -> Value {
    json!({
        "ItemID": id,
        "Revision": revision,
        "ContentFormatVersion": 8,
        "KeyRotation": 1,
        "Content": content,
        "ItemKey": item_key,
        "State": 1,
        "CreateTime": 1_700_000_000i64,
        "ModifyTime": 1_700_000_100i64,
        "RevisionTime": 1_700_000_100i64,
        "Pinned": false,
        "Flags": 0,
        "ShareCount": 0,
        "FolderID": folder
    })
}

pub(crate) fn passphrase() -> Vec<u8> {
    let salt = BASE64_STANDARD.decode(SALT).unwrap();

    Srp::new()
        .mailbox_passphrase(PASSWORD, salt)
        .unwrap()
        .as_bytes()
        .to_vec()
}

#[derive(Clone)]
pub(crate) struct KeyMaterial {
    pub(crate) user: PrivateKey,
    pub(crate) address: PrivateKey,
    pub(crate) share_key: EncryptionKey,
    pub(crate) item_key: EncryptionKey,
}

impl KeyMaterial {
    pub(crate) fn new() -> KeyMaterial {
        KeyMaterial {
            user: new_key(EMAIL),
            address: new_key(EMAIL),
            share_key: EncryptionKey::generate(),
            item_key: EncryptionKey::generate(),
        }
    }

    pub(crate) fn user_keys(&self) -> UserKeys {
        UserKeys::new(
            KeyRing::new().with_private(self.user.clone()),
            self.user.clone(),
        )
    }

    pub(crate) fn item(&self, revision: i64) -> Value {
        revision_json(
            ITEM,
            revision,
            &seal_base64(
                &self.item_key,
                login("Proton", "hunter2").encode(),
                &EncryptionTag::ItemContent,
            ),
            &seal_base64(
                &self.share_key,
                self.item_key.as_bytes(),
                &EncryptionTag::ItemKey,
            ),
            None,
        )
    }
}

pub(crate) async fn fixture<R>(routes: R) -> Fixture
where
    R: Fn(&Request) -> Option<ResponseFuture> + Send + Sync + 'static,
{
    seeded(KeyMaterial::new(), routes).await
}

pub(crate) async fn seeded<R>(seeds: KeyMaterial, routes: R) -> Fixture
where
    R: Fn(&Request) -> Option<ResponseFuture> + Send + Sync + 'static,
{
    let KeyMaterial {
        user,
        address,
        share_key,
        item_key,
    } = seeds.clone();
    let record = Record::default();
    let keys = seeds.user_keys();

    let base = Base {
        user_key: lock_key(&user, passphrase()),
        address_key: lock_key(&address, passphrase()),
        share: json!({"Code": 1000, "Share": share_json(SHARE, &share_key)}).to_string(),
        share_keys: share_keys_json(&keys, &share_key).to_string(),
        item: json!({"Code": 1000, "Item": seeds.item(3)}).to_string(),
        item_keys: json!({
            "Code": 1000,
            "Keys": {
                "Total": 1,
                "Keys": [{
                    "KeyRotation": 1,
                    "Key": seal_base64(&share_key, item_key.as_bytes(), &EncryptionTag::ItemKey)
                }]
            }
        })
        .to_string(),
    };

    let captured = record.clone();
    let routes = Arc::new(routes);
    let account = Account::new(
        api(move |request: Request| {
            captured.push(&request);
            routes(&request).unwrap_or_else(|| base.respond(&request))
        })
        .await,
    );

    let keys = Arc::new(account.unlock(PASSWORD).await.unwrap());
    let client = PassClient::for_account(&account, &keys).unwrap();

    Fixture {
        client,
        user,
        address,
        share_key,
        item_key,
        record,
    }
}

struct Base {
    user_key: String,
    address_key: String,
    share: String,
    share_keys: String,
    item: String,
    item_keys: String,
}

impl Base {
    fn respond(&self, request: &Request) -> ResponseFuture {
        let share = format!("/pass/v1/share/{SHARE}");
        let share_keys = format!("/pass/v1/share/{SHARE}/key");
        let item = format!("/pass/v1/share/{SHARE}/item/{ITEM}");
        let item_keys = format!("/pass/v1/share/{SHARE}/item/{ITEM}/key");

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
                            "PrivateKey": self.user_key,
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
                        "ID": ADDRESS,
                        "Email": EMAIL,
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
                            "PrivateKey": self.address_key,
                            "Primary": 1,
                            "Active": 1
                        }]
                    }]
                })
                .to_string(),
            ),
            ("GET", "/core/v4/keys/salts") => response(
                200,
                json!({"Code": 1000, "KeySalts": [{"ID": "user-key", "KeySalt": SALT}]})
                    .to_string(),
            ),
            ("GET", path) if path == share => response(200, self.share.clone()),
            ("GET", path) if path == share_keys => response(200, self.share_keys.clone()),
            ("GET", path) if path == item => response(200, self.item.clone()),
            ("GET", path) if path == item_keys => response(200, self.item_keys.clone()),
            _ => not_found(),
        }
    }
}

pub(crate) fn public_keys(keys: &[&PrivateKey]) -> ResponseFuture {
    let listed: Vec<Value> = keys
        .iter()
        .map(|key| {
            json!({
                "Flags": 3,
                "PublicKey": key.public_key().export_armored().unwrap()
            })
        })
        .collect();

    response(
        200,
        json!({"Code": 1000, "Address": {"Keys": listed}}).to_string(),
    )
}
