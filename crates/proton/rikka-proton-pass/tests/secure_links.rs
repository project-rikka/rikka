use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::prelude::{BASE64_URL_SAFE_NO_PAD, Engine as _};
use rikka_proton_pass::{
    EncryptionKey, EncryptionTag, Error, FileMetadata, NewSecureLink, SecureLinkViewer,
};
use serde_json::{Value, json};

use crate::support::{ITEM, KeyMaterial, SHARE, decrypt, login, seal_base64, seeded};
use rikka_test_proton::{Request, response};

const URL: &str = "https://pass.proton.me/u/token-value";

#[tokio::test(flavor = "multi_thread")]
async fn secure_link_key_in_fragment() {
    let seeds = KeyMaterial::new();
    let file_key = EncryptionKey::generate();
    let created: Arc<Mutex<Option<Value>>> = Arc::default();

    let fixture = {
        let (seeds, file_key, created) = (seeds.clone(), file_key.clone(), created.clone());
        seeded(seeds.clone(), move |request: &Request| {
            let create = format!("/pass/v1/share/{SHARE}/item/{ITEM}/public_link");
            let created_link = created.lock().unwrap().clone();
            match request.route() {
                ("POST", path) if path == create => {
                    *created.lock().unwrap() = Some(request.json());
                    Some(response(200,
                        json!({
                            "Code": 1000,
                            "PublicLink": {
                                "Url": URL,
                                "PublicLinkID": "link-value",
                                "ExpirationTime": 1_800_000_000i64
                            }
                        })
                        .to_string(),
                    ))
                }
                ("GET", "/pass/v1/public_link") => Some(response(200,
                    json!({
                        "Code": 1000,
                        "PublicLinks": [{
                            "LinkID": "link-value",
                            "ReadCount": 1,
                            "MaxReadCount": 3,
                            "ExpirationTime": 1_800_000_000i64,
                            "ShareID": SHARE,
                            "ItemID": ITEM,
                            "LinkURL": URL,
                            "EncryptedLinkKey": created_link?["EncryptedLinkKey"],
                            "LinkKeyShareKeyRotation": 1,
                            "Active": true,
                            "LinkKeyEncryptedWithItemKey": true
                        }]
                    })
                    .to_string(),
                )),
                ("GET", "/pass/v1/public_link/content/token-value") => Some(response(200,
                    json!({
                        "Code": 1000,
                        "PublicLinkContent": {
                            "Contents": seal_base64(
                                &seeds.item_key,
                                login("Proton", "hunter2").encode(),
                                &EncryptionTag::ItemContent
                            ),
                            "ItemKey": created_link?["EncryptedItemKey"],
                            "FilesToken": "files-value",
                            "ContentFormatVersion": 8,
                            "ReadCount": 1,
                            "MaxReadCount": 3,
                            "ExpirationTime": 1_800_000_000i64
                        }
                    })
                    .to_string(),
                )),
                ("GET", "/pass/v1/public_link/files/files-value") => {
                    let metadata = FileMetadata {
                        name: "scan.pdf".to_owned(),
                        mime_type: "application/pdf".to_owned(),
                    };
                    Some(response(200,
                        json!({
                            "Code": 1000,
                            "Files": {
                                "Files": [{
                                    "FileID": "file-value",
                                    "Size": 10,
                                    "Metadata": seal_base64(&file_key, metadata.encode(), &EncryptionTag::FileMetadata),
                                    "FileKey": seal_base64(&seeds.item_key, file_key.as_bytes(), &EncryptionTag::FileKey),
                                    "EncryptionVersion": 2,
                                    "ItemKeyRotation": 1,
                                    "Chunks": [{"ChunkID": "chunk-0", "Index": 0, "Size": 10}],
                                    "RevisionAdded": 1,
                                    "CreateTime": 1,
                                    "ModifyTime": 1
                                }],
                                "Total": 1
                            }
                        })
                        .to_string(),
                    ))
                }
                _ => None,
            }
        })
        .await
    };

    let item = fixture.item().await;
    let link = fixture
        .client
        .create_secure_link(
            &item,
            NewSecureLink::new(Duration::from_secs(7 * 24 * 3600)).max_reads(3),
        )
        .await
        .unwrap();

    let (base, fragment) = link.address().split_once('#').unwrap();
    assert_eq!(base, URL);
    let link_key = BASE64_URL_SAFE_NO_PAD.decode(fragment).unwrap();

    let request = created.lock().unwrap().clone().unwrap();
    assert_eq!(request["ExpirationTime"], 7 * 24 * 3600);
    assert_eq!(request["MaxReadCount"], 3);
    assert_eq!(request["Revision"], 3);
    assert_eq!(request["LinkKeyShareKeyRotation"], 1);
    assert_eq!(request["LinkKeyEncryptedWithItemKey"], true);
    assert_eq!(
        decrypt(
            &seeds.item_key,
            &request["EncryptedLinkKey"],
            &EncryptionTag::LinkKey
        ),
        link_key
    );
    assert_eq!(
        decrypt(
            &EncryptionKey::from_bytes(&link_key).unwrap(),
            &request["EncryptedItemKey"],
            &EncryptionTag::ItemKey
        ),
        seeds.item_key.as_bytes()
    );

    let listed = fixture.client.secure_links().await.unwrap();
    assert_eq!(listed[0].address(), link.address());
    assert_eq!(listed[0].max_read_count(), Some(3));

    let viewer = SecureLinkViewer::new(fixture.client.api().clone());
    let shown = viewer.open(link.address()).await.unwrap();
    assert_eq!(shown.item().name(), "Proton");
    assert_eq!(shown.files()[0].name(), "scan.pdf");
    assert_eq!(shown.files()[0].key().as_bytes(), file_key.as_bytes());

    let error = viewer.open(URL).await.unwrap_err();
    assert!(matches!(error, Error::MalformedLink(_)));
}
