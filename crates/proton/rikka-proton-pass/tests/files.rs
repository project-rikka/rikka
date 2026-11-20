use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use base64::prelude::{BASE64_STANDARD, Engine as _};
use rikka_proton_pass::{EncryptionKey, EncryptionTag, Error, FileMetadata};
use serde_json::{Value, json};

use crate::support::{ITEM, KeyMaterial, SHARE, decrypt, seal_base64, seeded};
use rikka_test_proton::{Request, ResponseFuture, binary_response, response, success};

const FILE: &str = "file-value";
const CHUNK: usize = 4 * 1024 * 1024;

#[derive(Default)]
struct Stored {
    created: Option<Value>,
    chunks: BTreeMap<usize, Vec<u8>>,
    file_key: Option<String>,
}

fn chunk_part(request: &Request) -> (usize, Vec<u8>) {
    let index = request
        .field("ChunkIndex")
        .and_then(|index| String::from_utf8_lossy(&index).parse().ok())
        .unwrap();
    let data = request.field("ChunkData").unwrap();

    (index, data)
}

fn file_json(stored: &Stored, rotation: Value) -> Value {
    let created = stored.created.as_ref().unwrap();
    let chunks: Vec<Value> = stored
        .chunks
        .iter()
        .rev()
        .map(|(index, data)| json!({"ChunkID": format!("chunk-{index}"), "Index": index, "Size": data.len()}))
        .collect();

    json!({
        "FileID": FILE,
        "PersistentFileUID": "uid-value",
        "Size": 0,
        "Metadata": created["Metadata"],
        "FileKey": stored.file_key,
        "EncryptionVersion": created["EncryptionVersion"],
        "ItemKeyRotation": rotation,
        "Chunks": chunks,
        "RevisionAdded": 4,
        "RevisionRemoved": null,
        "CreateTime": 1,
        "ModifyTime": 1
    })
}

fn file_routes(
    seeds: &KeyMaterial,
    stored: &Arc<Mutex<Stored>>,
    swapped: &Arc<AtomicBool>,
) -> impl Fn(&Request) -> Option<ResponseFuture> + Send + Sync + 'static {
    let (seeds, stored, swapped) = (seeds.clone(), stored.clone(), swapped.clone());
    let link = format!("/pass/v1/share/{SHARE}/item/{ITEM}/link_files");
    let files = format!("/pass/v1/share/{SHARE}/item/{ITEM}/files");
    let chunk = format!("/pass/v1/share/{SHARE}/item/{ITEM}/file/{FILE}/chunk/chunk-");

    move |request: &Request| {
        let mut stored = stored.lock().unwrap();
        match request.route() {
            ("POST", "/pass/v1/file") => {
                stored.created = Some(request.json());
                Some(response(
                    200,
                    json!({"Code": 1000, "File": {"FileID": FILE}}).to_string(),
                ))
            }
            ("POST", path) if path == format!("/pass/v1/file/{FILE}/chunk") => {
                let (index, data) = chunk_part(request);
                stored.chunks.insert(index, data);
                Some(success())
            }
            ("POST", path) if path == link => {
                stored.file_key = request.json()["FilesToAdd"][0]["FileKey"]
                    .as_str()
                    .map(str::to_owned);
                Some(response(
                    200,
                    json!({"Code": 1000, "Item": seeds.item(4)}).to_string(),
                ))
            }
            ("GET", path) if path == files => Some(response(
                200,
                json!({
                    "Code": 1000,
                    "Files": {"Files": [file_json(&stored, json!("1"))], "Total": 1, "LastID": null}
                })
                .to_string(),
            )),
            ("GET", path) if path.starts_with(&chunk) => {
                let index: usize = path[chunk.len()..].parse().unwrap();
                let served = match swapped.load(Ordering::SeqCst) {
                    true => stored.chunks.len() - 1 - index,
                    false => index,
                };
                Some(binary_response(200, stored.chunks[&served].clone()))
            }
            _ => None,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn attachment_chunks_round_trip() {
    let seeds = KeyMaterial::new();
    let stored = Arc::new(Mutex::new(Stored::default()));
    let swapped = Arc::new(AtomicBool::new(false));
    let fixture = seeded(seeds.clone(), file_routes(&seeds, &stored, &swapped)).await;

    let data: Vec<u8> = (0..CHUNK + 1000).map(|at| (at % 251) as u8).collect();
    let pending = fixture
        .client
        .attach("notes.txt")
        .upload_bytes(data.clone())
        .await
        .unwrap();
    assert_eq!(pending.chunk_count(), 2);
    assert_eq!(pending.mime_type(), Some("text/plain"));

    {
        let stored_files = stored.lock().unwrap();
        let created = stored_files.created.as_ref().unwrap();
        assert_eq!(created["ChunkCount"], 2);
        assert_eq!(created["EncryptionVersion"], 2);

        let metadata = FileMetadata::decode(&decrypt(
            pending.key(),
            &created["Metadata"],
            &EncryptionTag::FileMetadata,
        ))
        .unwrap();
        assert_eq!(metadata.name, "notes.txt");

        let first = pending
            .key()
            .open(
                &stored_files.chunks[&0],
                &EncryptionTag::FileChunk {
                    index: 0,
                    chunks: 2,
                },
            )
            .unwrap();
        assert_eq!(first, data[..CHUNK]);
    }

    let item = fixture.item().await;
    let linked = fixture
        .client
        .link_files(&item, std::slice::from_ref(&pending), &[])
        .await
        .unwrap();
    assert_eq!(linked.revision(), 4);

    let file_key = decrypt(
        &seeds.item_key,
        &json!(stored.lock().unwrap().file_key),
        &EncryptionTag::FileKey,
    );
    assert_eq!(file_key, pending.key().as_bytes());

    let files = fixture.client.files(&linked).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name(), "notes.txt");

    let downloaded = fixture
        .client
        .download_file(&files[0])
        .into_vec()
        .await
        .unwrap();
    assert_eq!(downloaded, data);

    swapped.store(true, Ordering::SeqCst);
    let error = fixture
        .client
        .download_file(&files[0])
        .into_vec()
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Undecryptable));
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_empty_attachment() {
    let fixture = seeded(KeyMaterial::new(), |_: &Request| None).await;

    let error = fixture
        .client
        .attach("empty.txt")
        .upload_bytes(Vec::new())
        .await
        .unwrap_err();

    assert!(matches!(error, Error::EmptyFile));
    assert!(fixture.record.matching("POST", "/pass/v1/file").is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn linked_file_rename_and_restore() {
    let seeds = KeyMaterial::new();
    let file_key = EncryptionKey::generate();
    let metadata = FileMetadata {
        name: "scan.pdf".to_owned(),
        mime_type: "application/pdf".to_owned(),
    };
    let listed = json!({
        "FileID": FILE,
        "PersistentFileUID": "uid-value",
        "Size": 10,
        "Metadata": seal_base64(&file_key, metadata.encode(), &EncryptionTag::FileMetadata),
        "FileKey": seal_base64(&seeds.item_key, file_key.as_bytes(), &EncryptionTag::FileKey),
        "EncryptionVersion": 2,
        "ItemKeyRotation": 1,
        "Chunks": [{"ChunkID": "chunk-0", "Index": 0, "Size": 10}],
        "RevisionAdded": 2,
        "RevisionRemoved": 3,
        "CreateTime": 1,
        "ModifyTime": 1
    });

    let fixture = {
        let (seeds, listed) = (seeds.clone(), listed.clone());
        seeded(seeds.clone(), move |request: &Request| {
            let history = format!("/pass/v1/share/{SHARE}/item/{ITEM}/revisions/files");
            let rename = format!("/pass/v1/share/{SHARE}/item/{ITEM}/file/{FILE}/metadata");
            let restore = format!("/pass/v1/share/{SHARE}/item/{ITEM}/file/restore");
            match request.route() {
                ("GET", path) if path == history => Some(response(200,
                    json!({"Code": 1000, "Files": {"Files": [listed], "Total": 1}}).to_string(),
                )),
                ("PUT", path) if path == rename => {
                    let mut renamed = listed.clone();
                    renamed["Metadata"] = request.json()["Metadata"].clone();
                    Some(response(200, json!({"Code": 1000, "File": renamed}).to_string()))
                }
                ("POST", path) if path == restore => {
                    let mut restored = listed.clone();
                    restored["FileID"] = json!("restored-value");
                    restored["FileKey"] = request.json()["FilesToRestore"][0]["FileKey"].clone();
                    restored["RevisionRemoved"] = Value::Null;
                    Some(response(200,
                        json!({"Code": 1000, "Result": {"Item": seeds.item(5), "Files": [restored]}})
                            .to_string(),
                    ))
                }
                _ => None,
            }
        })
        .await
    };

    let item = fixture.item().await;
    let history = fixture.client.file_history(&item).await.unwrap();
    assert_eq!(history[0].name(), "scan.pdf");
    assert!(history[0].is_present_in(2));
    assert!(!history[0].is_present_in(3));

    let renamed = fixture
        .client
        .rename_file(&history[0], "receipt.pdf")
        .await
        .unwrap();
    assert_eq!(renamed.name(), "receipt.pdf");
    assert_eq!(renamed.mime_type(), Some("application/pdf"));

    let (restored_item, restored) = fixture.client.restore_files(&item, &history).await.unwrap();
    assert_eq!(restored_item.revision(), 5);
    assert_eq!(restored[0].id().as_str(), "restored-value");
    assert_eq!(restored[0].key().as_bytes(), file_key.as_bytes());

    let request = fixture
        .record
        .last(
            "POST",
            &format!("/pass/v1/share/{SHARE}/item/{ITEM}/file/restore"),
        )
        .json();
    assert_eq!(request["ItemKeyRotation"], 1);
    let decoded = BASE64_STANDARD
        .decode(request["FilesToRestore"][0]["FileKey"].as_str().unwrap())
        .unwrap();
    assert!(
        seeds
            .item_key
            .open(decoded, &EncryptionTag::FileKey)
            .is_ok()
    );
}
