use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_account::{Account, UserKeys};
use rikka_proton_pass::{
    EncryptionKey, EncryptionTag, Item, ItemContent, ItemId, ItemLogin, ItemState, Metadata,
    NewItem, NewVault, PassClient, ShareId, Vault,
};
use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey};
use rikka_test_proton::{EMAIL, Record, Request, api, lock_key, new_key, not_found, response};
use serde_json::{Value, json};

use crate::support::{PASSWORD, SALT, passphrase};

const SHARE: &str = "share-value";
const ITEM: &str = "item-value";
const FOLDER_ITEM: &str = "filed-value";
const ITEM_SHARE: &str = "item-share-value";
const FOLDER: &str = "folder-value";

struct Fixture {
    client: PassClient,
    user: PrivateKey,
    share_key: EncryptionKey,
    newer_key: EncryptionKey,
    rotated: Arc<AtomicBool>,
    record: Record,
}

impl Fixture {
    fn share(&self) -> ShareId {
        ShareId::from(SHARE)
    }

    fn user_keys(&self) -> UserKeys {
        UserKeys::new(
            KeyRing::new().with_private(self.user.clone()),
            self.user.clone(),
        )
    }
}

fn login(name: &str, password: &str) -> Item {
    Item::new(
        Metadata {
            name: name.to_owned(),
            note: "a note".to_owned(),
            item_uuid: "uuid".to_owned(),
            custom_icon: None,
        },
        ItemContent::Login(ItemLogin {
            item_email: "tester@proton.test".to_owned(),
            password: password.to_owned(),
            ..ItemLogin::default()
        }),
    )
}

fn seal_base64(key: &EncryptionKey, data: impl AsRef<[u8]>, tag: &EncryptionTag) -> String {
    BASE64_STANDARD.encode(key.seal(data, tag).unwrap())
}

fn share_response(content: &str) -> Value {
    share_response_at(content, 2)
}

fn share_response_at(content: &str, rotation: i64) -> Value {
    json!({
        "ShareID": SHARE,
        "VaultID": "vault-value",
        "AddressID": "address-value",
        "TargetType": 1,
        "TargetID": "vault-value",
        "Permission": 0,
        "Content": content,
        "ContentKeyRotation": rotation,
        "ContentFormatVersion": 1,
        "ShareRoleID": "1",
        "TargetMaxMembers": 10,
        "TargetMembers": 1,
        "PendingInvites": 0,
        "NewUserInvitesReady": 0,
        "Owner": true,
        "Shared": false,
        "CanAutoFill": true,
        "CreateTime": 1_700_000_000i64,
        "Flags": 0
    })
}

fn revision(id: &str, rotation: i64, content: &str, key: &str, folder: Option<&str>) -> Value {
    json!({
        "ItemID": id,
        "Revision": 3,
        "ContentFormatVersion": 8,
        "KeyRotation": rotation,
        "Content": content,
        "ItemKey": key,
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

async fn fixture() -> Fixture {
    let pgp = Pgp::new();
    let user = new_key(&pgp, EMAIL);
    let address = new_key(&pgp, EMAIL);
    let record = Record::default();

    let share_key = EncryptionKey::generate();
    let older_key = EncryptionKey::generate();
    let newer_key = EncryptionKey::generate();
    let rotated = Arc::new(AtomicBool::new(false));
    let folder_key = EncryptionKey::generate();
    let item_key = EncryptionKey::generate();
    let filed_key = EncryptionKey::generate();
    let shared_item_key = EncryptionKey::generate();

    let keys = UserKeys::new(KeyRing::new().with_private(user.clone()), user.clone());

    let vault = Vault {
        name: "Personal".to_owned(),
        description: "the first vault".to_owned(),
        display: None,
    };
    let folder = rikka_proton_pass::Folder {
        name: "Work".to_owned(),
    };

    let responses = Responses {
        user_key: lock_key(&user, passphrase()),
        address_key: lock_key(&address, passphrase()),
        share: share_response(&seal_base64(
            &share_key,
            vault.encode(),
            &EncryptionTag::VaultContent,
        ))
        .to_string(),
        rotated: Arc::clone(&rotated),
        rotated_share: share_response_at(
            &seal_base64(&newer_key, vault.encode(), &EncryptionTag::VaultContent),
            3,
        )
        .to_string(),
        rotated_share_keys: json!({
            "Code": 1000,
            "ShareKeys": {
                "Total": 3,
                "Keys": [
                    {
                        "KeyRotation": 1,
                        "Key": older_key.wrap(&Pgp::new(), &keys).unwrap(),
                        "CreateTime": 1
                    },
                    {
                        "KeyRotation": 2,
                        "Key": share_key.wrap(&Pgp::new(), &keys).unwrap(),
                        "CreateTime": 2
                    },
                    {
                        "KeyRotation": 3,
                        "Key": newer_key.wrap(&Pgp::new(), &keys).unwrap(),
                        "CreateTime": 3
                    }
                ]
            }
        })
        .to_string(),
        share_keys: json!({
            "Code": 1000,
            "ShareKeys": {
                "Total": 2,
                "Keys": [
                    {
                        "KeyRotation": 1,
                        "Key": older_key.wrap(&Pgp::new(), &keys).unwrap(),
                        "CreateTime": 1
                    },
                    {
                        "KeyRotation": 2,
                        "Key": share_key.wrap(&Pgp::new(), &keys).unwrap(),
                        "CreateTime": 2
                    }
                ]
            }
        })
        .to_string(),
        items: json!({
            "Code": 1000,
            "Items": {
                "Total": 2,
                "LastToken": null,
                "RevisionsData": [
                    revision(
                        ITEM,
                        2,
                        &seal_base64(&item_key, login("Proton", "hunter2").encode(), &EncryptionTag::ItemContent),
                        &seal_base64(&share_key, item_key.as_bytes(), &EncryptionTag::ItemKey),
                        None
                    ),
                    revision(
                        FOLDER_ITEM,
                        2,
                        &seal_base64(&filed_key, login("Work", "hunter3").encode(), &EncryptionTag::ItemContent),
                        &seal_base64(&folder_key, filed_key.as_bytes(), &EncryptionTag::ItemKey),
                        Some(FOLDER)
                    )
                ]
            }
        })
        .to_string(),
        folders: json!({
            "Code": 1000,
            "Folders": {
                "Total": 1,
                "LastToken": null,
                "Folders": [{
                    "VaultID": "vault-value",
                    "FolderID": FOLDER,
                    "ParentFolderID": null,
                    "KeyRotation": 2,
                    "FolderKey": seal_base64(&share_key, folder_key.as_bytes(), &EncryptionTag::FolderKey),
                    "ContentFormatVersion": 1,
                    "Content": seal_base64(&folder_key, folder.encode(), &EncryptionTag::FolderContent)
                }]
            }
        })
        .to_string(),
        events: json!({
            "Code": 1000,
            "Events": {
                "UpdatedShare": null,
                "UpdatedItems": [revision(
                    ITEM,
                    2,
                    &seal_base64(&item_key, login("Proton", "hunter4").encode(), &EncryptionTag::ItemContent),
                    &seal_base64(&share_key, item_key.as_bytes(), &EncryptionTag::ItemKey),
                    None
                )],
                "DeletedItemIDs": ["gone-value"],
                "NewKeyRotation": 3,
                "LatestEventID": "event-2",
                "EventsPending": false
            }
        })
        .to_string(),
        item_share_keys: json!({
            "Code": 1000,
            "ShareKeys": {
                "Total": 1,
                "Keys": [{
                    "KeyRotation": 1,
                    "Key": shared_item_key.wrap(&Pgp::new(), &keys).unwrap(),
                    "CreateTime": 1
                }]
            }
        })
        .to_string(),
        item_share_items: json!({
            "Code": 1000,
            "Items": {
                "Total": 1,
                "LastToken": null,
                "RevisionsData": [{
                    "ItemID": ITEM,
                    "Revision": 1,
                    "ContentFormatVersion": 8,
                    "KeyRotation": 1,
                    "Content": seal_base64(
                        &shared_item_key,
                        login("Shared", "hunter7").encode(),
                        &EncryptionTag::ItemContent
                    ),
                    "ItemKey": null,
                    "State": 1,
                    "CreateTime": 1_700_000_000i64,
                    "ModifyTime": 1_700_000_000i64,
                    "RevisionTime": 1_700_000_000i64,
                    "Pinned": false,
                    "Flags": 0,
                    "ShareCount": 1
                }]
            }
        })
        .to_string(),
    };

    let captured = record.clone();
    let account = Account::new(
        api(move |request: Request| {
            captured.push(&request);
            responses.respond(&request)
        })
        .await,
    );

    let decrypted = Arc::new(account.unlock(PASSWORD).await.unwrap());
    let client = PassClient::for_account(&account, &decrypted).unwrap();

    Fixture {
        client,
        user,
        share_key,
        newer_key,
        rotated,
        record,
    }
}

struct Responses {
    user_key: String,
    address_key: String,
    share: String,
    share_keys: String,
    rotated: Arc<AtomicBool>,
    rotated_share: String,
    rotated_share_keys: String,
    items: String,
    folders: String,
    events: String,
    item_share_keys: String,
    item_share_items: String,
}

impl Responses {
    fn respond(&self, request: &Request) -> rikka_test_proton::ResponseFuture {
        let share_key = format!("/pass/v1/share/{SHARE}/key");
        let share_one = format!("/pass/v1/share/{SHARE}");
        let item_path = format!("/pass/v1/share/{SHARE}/item");
        let item_one = format!("/pass/v1/share/{SHARE}/item/{ITEM}");
        let folder_path = format!("/pass/v1/share/{SHARE}/folder");
        let trash_path = format!("/pass/v1/share/{SHARE}/item/trash");
        let events_path = format!("/pass/v1/share/{SHARE}/event/event-1");
        let alias_path = format!("/pass/v1/share/{SHARE}/alias/options");
        let pin_path = format!("/pass/v1/share/{SHARE}/item/{ITEM}/pin");
        let flags_path = format!("/pass/v1/share/{SHARE}/item/{ITEM}/flags");
        let address_breaches = "/pass/v1/breach/address/address-value/breaches";
        let item_share_key = format!("/pass/v1/share/{ITEM_SHARE}/key");
        let item_share_items = format!("/pass/v1/share/{ITEM_SHARE}/item");

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
                        "ID": "address-value",
                        "Email": "tester@proton.test",
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
            ("GET", "/pass/v1/share") => response(
                200,
                json!({"Code": 1000, "Shares": [self.current_share()]}).to_string(),
            ),
            ("GET", path) if path == share_one => response(
                200,
                json!({"Code": 1000, "Share": self.current_share()}).to_string(),
            ),
            ("GET", path) if path == share_key => response(
                200,
                match self.is_rotated() {
                    true => self.rotated_share_keys.clone(),
                    false => self.share_keys.clone(),
                },
            ),
            ("GET", path) if path == item_share_key => response(200, self.item_share_keys.clone()),
            ("GET", path) if path == item_share_items => {
                response(200, self.item_share_items.clone())
            }
            ("GET", path) if path == item_path => response(200, self.items.clone()),
            ("GET", path) if path == item_one => response(
                200,
                json!({"Code": 1000, "Item": self.held_item(false, 0)}).to_string(),
            ),
            ("GET", path) if path == folder_path => response(200, self.folders.clone()),
            ("GET", path) if path == events_path => response(200, self.events.clone()),
            ("GET", path) if path == alias_path => response(
                200,
                json!({
                    "Code": 1000,
                    "Options": {
                        "Suffixes": [{
                            "Suffix": ".some@proton.me",
                            "SignedSuffix": "signed",
                            "IsCustom": false,
                            "IsPremium": false,
                            "Domain": "proton.me"
                        }],
                        "Mailboxes": [{"ID": 4, "Email": "tester@proton.test"}]
                    }
                })
                .to_string(),
            ),
            ("GET", "/pass/v1/user/access") => response(
                200,
                json!({
                    "Code": 1000,
                    "Access": {
                        "Plan": {
                            "Type": "plus",
                            "InternalName": "pass2023",
                            "DisplayName": "Pass Plus",
                            "HideUpgrade": false,
                            "ManageAlias": true,
                            "StorageAllowed": true,
                            "StorageUsed": 12,
                            "StorageQuota": 1024,
                            "StorageMaxFileSize": 512,
                            "FolderAllowed": true
                        },
                        "Monitor": {"ProtonAddress": true, "Aliases": false},
                        "PendingInvites": 0,
                        "WaitingNewUserInvites": 0,
                        "UserData": {
                            "DefaultShareID": SHARE,
                            "AliasSyncEnabled": false,
                            "PendingAliasToSync": 0
                        }
                    }
                })
                .to_string(),
            ),
            ("POST", "/pass/v1/vault") => {
                let request = request.json();
                let content = request["Content"].as_str().unwrap_or_default();
                response(
                    200,
                    json!({"Code": 1000, "Share": share_response(content)}).to_string(),
                )
            }
            ("POST", path) if path == item_path => {
                let request = request.json();
                response(
                    200,
                    json!({
                        "Code": 1000,
                        "Item": revision(
                            "created-value",
                            request["KeyRotation"].as_i64().unwrap_or_default(),
                            request["Content"].as_str().unwrap_or_default(),
                            request["ItemKey"].as_str().unwrap_or_default(),
                            request["FolderID"].as_str()
                        )
                    })
                    .to_string(),
                )
            }
            ("PUT", path) if path == item_one => {
                let request = request.json();
                response(
                    200,
                    json!({
                        "Code": 1000,
                        "Item": revision(
                            ITEM,
                            request["KeyRotation"].as_i64().unwrap_or_default(),
                            request["Content"].as_str().unwrap_or_default(),
                            "",
                            None
                        )
                    })
                    .to_string(),
                )
            }
            ("POST", path) if path == trash_path => response(
                200,
                json!({
                    "Code": 1000,
                    "Items": [{
                        "ItemID": ITEM,
                        "Revision": 4,
                        "State": 2,
                        "ModifyTime": 1_700_000_200i64,
                        "RevisionTime": 1_700_000_200i64,
                        "Flags": 0
                    }]
                })
                .to_string(),
            ),
            ("POST", path) if path == pin_path => response(
                200,
                json!({"Code": 1000, "Item": self.held_item(true, 0)}).to_string(),
            ),
            ("DELETE", path) if path == pin_path => response(
                200,
                json!({"Code": 1000, "Item": self.held_item(false, 0)}).to_string(),
            ),
            ("PUT", path) if path == flags_path => response(
                200,
                json!({"Code": 1000, "Item": self.held_item(false, 1)}).to_string(),
            ),
            ("GET", "/pass/v1/breach") => response(
                200,
                json!({
                    "Code": 1000,
                    "Breaches": {
                        "EmailsCount": 3,
                        "DomainsPeek": [{"Domain": "shop.example", "BreachTime": 1_700_000_000i64}],
                        "CustomEmails": [],
                        "Addresses": [{
                            "AddressID": "address-value",
                            "Email": "tester@proton.test",
                            "BreachCounter": 3,
                            "Flags": 0,
                            "LastBreachTime": 1_700_000_000i64
                        }],
                        "HasCustomDomains": false
                    }
                })
                .to_string(),
            ),
            ("GET", path) if path == address_breaches => response(
                200,
                json!({
                    "Code": 1000,
                    "Breaches": {
                        "IsEligible": true,
                        "Count": 1,
                        "Breaches": [{
                            "ID": "breach-value",
                            "Email": "tester@proton.test",
                            "ResolvedState": 1,
                            "Severity": 0.5,
                            "Name": "Shop",
                            "CreatedAt": "2024-01-01T00:00:00+00:00",
                            "PublishedAt": "2024-01-02T00:00:00+00:00",
                            "Source": {"IsAggregated": false, "Domain": "shop.example"},
                            "Size": 1200,
                            "ExposedData": [{"Code": "password", "Name": "Password"}],
                            "PasswordLastChars": null,
                            "Actions": []
                        }]
                    }
                })
                .to_string(),
            ),
            ("DELETE", path) if path == share_one => {
                response(200, json!({"Code": 1000}).to_string())
            }
            _ => not_found(),
        }
    }

    fn is_rotated(&self) -> bool {
        self.rotated.load(Ordering::SeqCst)
    }

    fn current_share(&self) -> Value {
        let share = match self.is_rotated() {
            true => &self.rotated_share,
            false => &self.share,
        };
        serde_json::from_str(share).unwrap()
    }

    fn held_item(&self, pinned: bool, flags: i64) -> Value {
        let page: Value = serde_json::from_str(&self.items).unwrap();
        let mut item = page["Items"]["RevisionsData"][0].clone();
        item["Pinned"] = json!(pinned);
        item["Flags"] = json!(flags);
        item
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn items_open_with_share_and_folder_keys() {
    let fixture = fixture().await;
    let items = fixture.client.items(&fixture.share()).await.unwrap();

    assert_eq!(items.len(), 2);

    let first_item = &items[0];
    assert_eq!(first_item.id(), &ItemId::from(ITEM));
    assert_eq!(first_item.name(), "Proton");
    assert_eq!(first_item.state(), ItemState::Active);
    assert_eq!(
        first_item.content().login().unwrap().password.as_str(),
        "hunter2"
    );

    let filed = &items[1];
    assert_eq!(filed.name(), "Work");
    assert_eq!(filed.folder_id().map(|id| id.as_str()), Some(FOLDER));
    assert_eq!(
        filed.content().login().unwrap().password.as_str(),
        "hunter3"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn created_vault_seals_key() {
    let fixture = fixture().await;
    let share = fixture
        .client
        .create_vault(NewVault::new("Personal").description("the first vault"))
        .await
        .unwrap();

    assert_eq!(share.name(), Some("Personal"));

    let body = fixture.record.last("POST", "/pass/v1/vault").json();
    assert_eq!(body["AddressID"], "address-value");
    assert_eq!(body["ContentFormatVersion"], 1);

    let key = EncryptionKey::unwrap(
        &Pgp::new(),
        &fixture.user_keys(),
        &fixture.share(),
        BASE64_STANDARD
            .decode(body["EncryptedVaultKey"].as_str().unwrap())
            .unwrap(),
    )
    .unwrap();

    let content = BASE64_STANDARD
        .decode(body["Content"].as_str().unwrap())
        .unwrap();
    let decrypted = key.open(content, &EncryptionTag::VaultContent).unwrap();

    assert_eq!(Vault::decode(&decrypted).unwrap().name, "Personal");
}

#[tokio::test(flavor = "multi_thread")]
async fn created_item_round_trips() {
    let fixture = fixture().await;
    let item = fixture
        .client
        .create_item(&fixture.share(), NewItem::new(login("Proton", "hunter5")))
        .await
        .unwrap();

    assert_eq!(item.id(), &ItemId::from("created-value"));
    assert_eq!(item.name(), "Proton");
    assert_eq!(item.content().login().unwrap().password.as_str(), "hunter5");

    let body = fixture
        .record
        .last("POST", &format!("/pass/v1/share/{SHARE}/item"))
        .json();
    assert_eq!(body["KeyRotation"], 2);
    assert_eq!(body["ContentFormatVersion"], 8);

    let item_key = fixture
        .share_key
        .open(
            BASE64_STANDARD
                .decode(body["ItemKey"].as_str().unwrap())
                .unwrap(),
            &EncryptionTag::ItemKey,
        )
        .unwrap();
    assert_eq!(item_key.len(), 32);
}

#[tokio::test(flavor = "multi_thread")]
async fn update_keeps_item_key() {
    let fixture = fixture().await;
    let items = fixture.client.items(&fixture.share()).await.unwrap();

    let updated = fixture
        .client
        .update_item(&items[0], &login("Proton", "hunter6"))
        .await
        .unwrap();

    assert_eq!(
        updated.content().login().unwrap().password.as_str(),
        "hunter6"
    );

    let body = fixture
        .record
        .last("PUT", &format!("/pass/v1/share/{SHARE}/item/{ITEM}"))
        .json();
    assert_eq!(body["LastRevision"], 3);
    assert_eq!(body["KeyRotation"], 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_account_without_pass() {
    let fixture = fixture().await;
    let error = fixture
        .client
        .item(&fixture.share(), &ItemId::from("missing-value"))
        .await
        .unwrap_err();

    assert!(matches!(error, rikka_proton_pass::Error::Api(_)));
}

#[tokio::test(flavor = "multi_thread")]
async fn leaving_share_drops_keys() {
    let fixture = fixture().await;
    let share = fixture.share();
    let keys_path = format!("/pass/v1/share/{SHARE}/key");

    fixture.client.share_keys(&share).await.unwrap();
    fixture.client.leave_share(&share).await.unwrap();
    fixture.client.share_keys(&share).await.unwrap();

    assert_eq!(fixture.record.matching("GET", &keys_path).len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn new_item_uses_latest_rotation() {
    let fixture = fixture().await;
    let share = fixture.share();
    fixture.client.items(&share).await.unwrap();

    fixture.rotated.store(true, Ordering::SeqCst);
    fixture
        .client
        .create_item(&share, NewItem::new(login("Proton", "hunter8")))
        .await
        .unwrap();

    let body = fixture
        .record
        .last("POST", &format!("/pass/v1/share/{SHARE}/item"))
        .json();
    assert_eq!(body["KeyRotation"], 3);
    fixture
        .newer_key
        .open(
            BASE64_STANDARD
                .decode(body["ItemKey"].as_str().unwrap())
                .unwrap(),
            &EncryptionTag::ItemKey,
        )
        .unwrap();
}
