use rikka_proton_pass::{
    EncryptionKey, EncryptionTag, Folder, FolderId, ImportItem, ItemState, NewItem, ShareId,
};
use serde_json::json;

use crate::support::{
    ITEM, KeyMaterial, SHARE, decrypt, login, revision_json, seal_base64, seeded, share_json,
    share_keys_json,
};
use rikka_test_proton::{Request, response};

const TARGET: &str = "target-value";
const FOLDER: &str = "folder-value";

#[tokio::test(flavor = "multi_thread")]
async fn move_reencrypts_item_keys() {
    let seeds = KeyMaterial::new();
    let destination_key = EncryptionKey::generate();
    let folder_key = EncryptionKey::generate();

    let fixture = {
        let (seeds, destination_key, folder_key) =
            (seeds.clone(), destination_key.clone(), folder_key.clone());
        seeded(seeds.clone(), move |request: &Request| {
            let target_share = format!("/pass/v1/share/{TARGET}");
            let target_keys = format!("/pass/v1/share/{TARGET}/key");
            let to_share = format!("/pass/v1/share/{SHARE}/item/share");
            let to_folder = format!("/pass/v1/share/{SHARE}/item/folder");
            let folders = format!("/pass/v1/share/{SHARE}/folder");
            match request.route() {
                ("GET", path) if path == target_share => Some(response(200,
                    json!({"Code": 1000, "Share": share_json(TARGET, &destination_key)}).to_string(),
                )),
                ("GET", path) if path == target_keys => Some(response(200,
                    share_keys_json(&seeds.user_keys(), &destination_key).to_string(),
                )),
                ("GET", path) if path == folders => Some(response(200,
                    json!({
                        "Code": 1000,
                        "Folders": {
                            "Total": 1,
                            "Folders": [{
                                "FolderID": FOLDER,
                                "VaultID": "vault-value",
                                "KeyRotation": 1,
                                "FolderKey": seal_base64(&seeds.share_key, folder_key.as_bytes(), &EncryptionTag::FolderKey),
                                "ContentFormatVersion": 1,
                                "Content": seal_base64(&folder_key, Folder { name: "Work".to_owned() }.encode(), &EncryptionTag::FolderContent)
                            }]
                        }
                    })
                    .to_string(),
                )),
                ("PUT", path) if path == to_share => Some(response(200,
                    json!({
                        "Code": 1000,
                        "Items": [revision_json(
                            ITEM,
                            4,
                            &seal_base64(&seeds.item_key, login("Proton", "hunter2").encode(), &EncryptionTag::ItemContent),
                            request.json()["Items"][0]["ItemKeys"][0]["Key"].as_str()?,
                            None,
                        )]
                    })
                    .to_string(),
                )),
                ("PUT", path) if path == to_folder => Some(response(200,
                    json!({
                        "Code": 1000,
                        "Items": [{
                            "ItemID": ITEM,
                            "Revision": 4,
                            "State": 1,
                            "Flags": 0,
                            "FolderID": FOLDER,
                            "ModifyTime": 2,
                            "RevisionTime": 2
                        }]
                    })
                    .to_string(),
                )),
                _ => None,
            }
        })
        .await
    };

    let item = fixture.item().await;
    let target = ShareId::from(TARGET);

    let moved = fixture
        .client
        .move_items(std::slice::from_ref(&item), &target, None)
        .await
        .unwrap();
    assert_eq!(moved[0].share_id(), &target);
    assert_eq!(moved[0].name(), "Proton");

    let request = fixture
        .record
        .last("PUT", &format!("/pass/v1/share/{SHARE}/item/share"))
        .json();
    assert_eq!(request["ShareID"], TARGET);
    assert_eq!(request["Items"][0]["ItemID"], ITEM);
    assert_eq!(request["Items"][0]["DestinationFolderID"], json!(null));
    assert_eq!(request["Items"][0]["ItemKeys"][0]["KeyRotation"], 1);
    assert_eq!(
        decrypt(
            &destination_key,
            &request["Items"][0]["ItemKeys"][0]["Key"],
            &EncryptionTag::ItemKey
        ),
        seeds.item_key.as_bytes()
    );

    let filed = fixture
        .client
        .move_items_to_folder(
            &fixture.share(),
            std::slice::from_ref(&item),
            Some(&FolderId::from(FOLDER)),
        )
        .await
        .unwrap();
    assert_eq!(filed[0].folder_id(), Some(&FolderId::from(FOLDER)));

    let request = fixture
        .record
        .last("PUT", &format!("/pass/v1/share/{SHARE}/item/folder"))
        .json();
    assert_eq!(request["FolderID"], FOLDER);
    assert_eq!(
        decrypt(
            &folder_key,
            &request["Items"][0]["ItemKeys"][0]["Key"],
            &EncryptionTag::ItemKey
        ),
        seeds.item_key.as_bytes()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn import_keeps_item_history() {
    let fixture = seeded(KeyMaterial::new(), |request: &Request| {
        let import = format!("/pass/v1/share/{SHARE}/item/import/batch");
        match request.route() {
            ("POST", path) if path == import => {
                let revisions: Vec<_> = request.json()["Items"]
                    .as_array()?
                    .iter()
                    .enumerate()
                    .map(|(at, imported)| {
                        let mut revision = revision_json(
                            &format!("imported-{at}"),
                            1,
                            imported["Item"]["Content"].as_str().unwrap_or_default(),
                            imported["Item"]["ItemKey"].as_str().unwrap_or_default(),
                            None,
                        );
                        if imported["Trashed"] == true {
                            revision["State"] = json!(2);
                        }
                        revision
                    })
                    .collect();
                Some(response(
                    200,
                    json!({
                        "Code": 1000,
                        "Revisions": {"RevisionsData": revisions, "Total": 2, "LastToken": null}
                    })
                    .to_string(),
                ))
            }
            _ => None,
        }
    })
    .await;

    let imported = fixture
        .client
        .import_items(
            &fixture.share(),
            [
                ImportItem::new(NewItem::new(login("Mail", "first")))
                    .created_at(jiff::Timestamp::from_second(1_600_000_000).unwrap())
                    .modified_at(jiff::Timestamp::from_second(1_650_000_000).unwrap()),
                ImportItem::new(NewItem::new(login("Bank", "second"))).in_trash(),
            ],
        )
        .await
        .unwrap();

    assert_eq!(imported.len(), 2);
    assert_eq!(imported[0].name(), "Mail");
    assert_eq!(imported[1].state(), ItemState::Trashed);

    let request = fixture
        .record
        .last("POST", &format!("/pass/v1/share/{SHARE}/item/import/batch"))
        .json();
    assert_eq!(request["Items"][0]["CreateTime"], 1_600_000_000i64);
    assert_eq!(request["Items"][0]["ModifyTime"], 1_650_000_000i64);
    assert_eq!(request["Items"][0]["Trashed"], false);
    assert_eq!(request["Items"][1]["Trashed"], true);
    assert_eq!(request["Items"][0]["Item"]["KeyRotation"], 1);
}
