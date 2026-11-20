use rikka_proton_drive::{Error, RevisionState, ShareId};
use rikka_test_proton_drive::{
    CONTENT, Conflict, Corruption, DRAFT_NAME, DummyDrive, FILE_NAME, SPLIT, STALE, Shape,
    THUMBNAIL, base64_digest, lookup_hash, sha1_hex,
};

const NOTES: &str = "notes.txt";
const FILES: &str = "POST /drive/v2/volumes/volume/files";
const BLOCKS: &str = "POST /drive/blocks";
const COMMIT: &str = "PUT /drive/v2/volumes/volume/files/*/revisions/*";
const DRAFTS_DELETED: &str = "POST /drive/v2/volumes/volume/delete_multiple";

async fn open_fixture_file(
    client: &rikka_proton_drive::DriveClient,
) -> (
    rikka_proton_drive::ShareKeys,
    rikka_proton_drive::Folder,
    rikka_proton_drive::File,
) {
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();
    let file = client
        .file(
            share.volume_id(),
            &rikka_proton_drive::LinkId::from("file"),
            &root,
        )
        .await
        .unwrap();

    (share, root, file)
}

#[tokio::test(flavor = "multi_thread")]
async fn download_verifies_manifest() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let (share, _, file) = open_fixture_file(&client).await;

    let read = client
        .download(share.volume_id(), &file)
        .into_vec()
        .await
        .unwrap();

    assert_eq!(read, CONTENT);

    let mut chunks: Vec<usize> = Vec::new();
    let uploaded_file = client
        .download(share.volume_id(), &file)
        .for_each_block(|chunk: Vec<u8>| {
            chunks.push(chunk.len());
            Ok(())
        })
        .await
        .unwrap();

    assert_eq!(uploaded_file, CONTENT.len() as u64);
    assert_eq!(chunks, [SPLIT, CONTENT.len() - SPLIT]);
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_block_hash_mismatch() {
    let drive = DummyDrive::with_shape(Shape::new().corrupted(Corruption::Block)).await;
    let client = drive.client().await;
    let (share, _, file) = open_fixture_file(&client).await;

    let refused = client
        .download(share.volume_id(), &file)
        .into_vec()
        .await
        .unwrap_err();

    assert!(
        matches!(refused, Error::BlockCorrupted { index } if index == 1),
        "{refused}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn revision_keeps_extended_attributes() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let (share, _, file) = open_fixture_file(&client).await;

    let revision = client
        .revision(share.volume_id(), &file, file.revision_id().unwrap())
        .await
        .unwrap();

    assert_eq!(revision.blocks().len(), 2);
    assert!(revision.signature().is_valid());
    assert_eq!(
        revision
            .attributes()
            .and_then(rikka_proton_drive::ExtendedAttributes::size),
        Some(CONTENT.len() as u64)
    );
    assert_eq!(
        revision
            .attributes()
            .map(rikka_proton_drive::ExtendedAttributes::block_sizes),
        Some([SPLIT as u64, (CONTENT.len() - SPLIT) as u64].as_slice())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn upload_round_trips_blocks() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();

    let uploaded_file = client
        .upload(&share, &root, NOTES)
        .upload_bytes(CONTENT.to_vec())
        .await
        .unwrap();

    assert_eq!(uploaded_file.name(), NOTES);
    assert_eq!(uploaded_file.media_type(), "text/plain");
    assert!(uploaded_file.signatures().all_valid());

    let body = drive.last_body(FILES);
    assert_eq!(
        body["Hash"].as_str(),
        Some(lookup_hash(drive.root_hash(), NOTES).as_str())
    );
    assert_eq!(body["ParentLinkID"].as_str(), Some("root"));
    assert!(body["ContentKeyPacket"].as_str().is_some());

    let blocks = drive.last_body(BLOCKS);
    let listed = blocks["BlockList"].as_array().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["Index"].as_u64(), Some(1));
    assert_eq!(
        listed[0]["Hash"].as_str(),
        Some(base64_digest(&drive.uploaded(1)).as_str())
    );
    assert_eq!(
        listed[0]["Size"].as_u64(),
        Some(drive.uploaded(1).len() as u64)
    );
    assert!(listed[0]["EncSignature"].as_str().is_some());

    let committed = drive.last_body(COMMIT);
    assert_eq!(
        committed["SignatureAddress"].as_str(),
        Some(rikka_test_proton::EMAIL)
    );
    assert!(committed["ManifestSignature"].as_str().is_some());

    let read = client
        .download(share.volume_id(), &uploaded_file)
        .into_vec()
        .await
        .unwrap();
    assert_eq!(read, CONTENT);

    let attributes = uploaded_file.attributes().unwrap();
    assert_eq!(attributes.size(), Some(CONTENT.len() as u64));
    assert_eq!(attributes.block_sizes(), [CONTENT.len() as u64]);
    assert_eq!(
        attributes.sha1(),
        Some(sha1_hex(CONTENT).as_str()),
        "digest does not cover plaintext"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn upload_round_trips_thumbnail() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();

    let uploaded_file = client
        .upload(&share, &root, NOTES)
        .thumbnail(THUMBNAIL.to_vec())
        .upload_bytes(CONTENT.to_vec())
        .await
        .unwrap();

    let listed = drive.last_body(BLOCKS)["ThumbnailList"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0]["Type"].as_u64(), Some(1));
    assert_eq!(
        listed[0]["Hash"].as_str(),
        Some(base64_digest(&drive.uploaded(0)).as_str())
    );
    assert_eq!(
        listed[0]["Size"].as_u64(),
        Some(drive.uploaded(0).len() as u64)
    );

    let read = client
        .thumbnail(share.volume_id(), &uploaded_file)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read, THUMBNAIL);

    let content = client
        .download(share.volume_id(), &uploaded_file)
        .into_vec()
        .await
        .unwrap();
    assert_eq!(content, CONTENT);
}

fn numbered_blocks() -> Vec<Vec<u8>> {
    (0..60u8)
        .map(|block| {
            (0..5)
                .map(|offset| block.wrapping_mul(7).wrapping_add(offset))
                .collect()
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn download_parallel_blocks_in_order() {
    let blocks = numbered_blocks();
    let drive = DummyDrive::with_shape(Shape::new().blocks(blocks.clone()).slow_blocks()).await;
    let client = drive.client().await;
    let (share, _, file) = open_fixture_file(&client).await;

    let mut received = Vec::new();
    let uploaded_file = client
        .download(share.volume_id(), &file)
        .for_each_block(|chunk: Vec<u8>| {
            received.push(chunk);
            Ok(())
        })
        .await
        .unwrap();

    assert_eq!(received, blocks);
    assert_eq!(uploaded_file, blocks.concat().len() as u64);
    assert!(drive.max_concurrent_block_reads() > 1);
    assert_eq!(
        drive.request_count("GET /drive/v2/volumes/volume/files/file/revisions/revision"),
        2
    );

    let listed = client
        .revision_info(share.volume_id(), file.id(), file.revision_id().unwrap())
        .await
        .unwrap();
    assert_eq!(listed.blocks().len(), blocks.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_mismatched_manifest() {
    let drive = DummyDrive::with_shape(Shape::new().corrupted(Corruption::Manifest)).await;
    let client = drive.client().await;
    let (share, _, file) = open_fixture_file(&client).await;

    let refused = client
        .download(share.volume_id(), &file)
        .into_vec()
        .await
        .unwrap_err();

    assert!(
        matches!(
            refused,
            Error::Signature {
                field: "manifest",
                ..
            }
        ),
        "{refused}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn upload_sends_block_verifiers() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let (share, root, _) = open_fixture_file(&client).await;

    client
        .upload(&share, &root, NOTES)
        .expected_sha1(sha1_hex(CONTENT).to_uppercase())
        .upload_bytes(CONTENT.to_vec())
        .await
        .unwrap();

    let sealed = drive.uploaded(1);
    let expected: Vec<u8> = drive
        .verification()
        .iter()
        .enumerate()
        .map(|(at, code)| code ^ sealed.get(at).copied().unwrap_or(0))
        .collect();
    let listed = &drive.last_body(BLOCKS)["BlockList"][0];
    assert_eq!(
        listed["Verifier"]["Token"].as_str(),
        Some(rikka_proton_api::base64::encode(expected).as_str())
    );
    assert_eq!(
        drive.last_body(COMMIT)["ChecksumVerified"].as_bool(),
        Some(true)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn sha1_mismatch_deletes_draft() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let (share, root, _) = open_fixture_file(&client).await;

    let refused = client
        .upload(&share, &root, NOTES)
        .expected_sha1("0000000000000000000000000000000000000000")
        .upload_bytes(CONTENT.to_vec())
        .await
        .unwrap_err();

    assert!(
        matches!(refused, Error::ChecksumMismatch { .. }),
        "{refused}"
    );
    assert_eq!(drive.request_count(COMMIT), 0, "unexpected commit");
    let deleted = drive.bodies(DRAFTS_DELETED);
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0]["LinkIDs"].as_array().map(Vec::len), Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_upload_deletes_draft_with_retry() {
    let drive = DummyDrive::with_shape(Shape::new().failing_upload().flaky_delete()).await;
    let client = drive.client().await;
    let (share, root, _) = open_fixture_file(&client).await;

    let refused = client
        .upload(&share, &root, NOTES)
        .upload_bytes(CONTENT.to_vec())
        .await
        .unwrap_err();

    assert!(matches!(refused, Error::Transfer(_)), "{refused}");
    let deleted = drive.bodies(DRAFTS_DELETED);
    assert_eq!(deleted.len(), 2);
    assert_eq!(deleted[0]["LinkIDs"], deleted[1]["LinkIDs"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn stale_draft_replaced_foreign_reported() {
    let drive = DummyDrive::with_shape(Shape::new().conflict(Conflict::OwnDraft)).await;
    let client = drive.client().await;
    let client = client.with_client_uid("rikka-sync");
    let (share, root, _) = open_fixture_file(&client).await;

    client
        .upload(&share, &root, DRAFT_NAME)
        .upload_bytes(CONTENT.to_vec())
        .await
        .unwrap();
    assert_eq!(
        drive.last_body(DRAFTS_DELETED)["LinkIDs"],
        serde_json::json!([STALE])
    );
    assert_eq!(
        drive.last_body(FILES)["ClientUID"].as_str(),
        Some("rikka-sync")
    );

    let foreign = DummyDrive::with_shape(Shape::new().conflict(Conflict::ForeignDraft)).await;
    let client = foreign.client().await;
    let client = client.with_client_uid("rikka-sync");
    let (share, root, _) = open_fixture_file(&client).await;
    let refused = client
        .upload(&share, &root, DRAFT_NAME)
        .upload_bytes(CONTENT.to_vec())
        .await
        .unwrap_err();
    assert!(
        matches!(&refused, Error::Conflict { link, draft: Some(_) } if link.as_str() == STALE),
        "{refused}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn new_revision_reuses_file_keys() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let (share, root, file) = open_fixture_file(&client).await;

    let revised = client
        .upload_revision(&share, &root, &file)
        .upload_bytes(b"the changed words".to_vec())
        .await
        .unwrap();

    assert_eq!(revised.id(), file.id());
    assert_eq!(
        drive.last_body("POST /drive/v2/volumes/volume/files/file/revisions")["CurrentRevisionID"]
            .as_str(),
        Some("revision")
    );
    assert!(
        drive.last_body(COMMIT)["ManifestSignature"]
            .as_str()
            .is_some()
    );
    let downloaded = drive
        .pgp()
        .decrypt()
        .with_session_key(drive.content().clone())
        .message(drive.uploaded(1))
        .unwrap();
    assert_eq!(downloaded.data(), b"the changed words");
}

#[tokio::test(flavor = "multi_thread")]
async fn revisions_list_restore_delete() {
    let drive = DummyDrive::with_shape(Shape::new().revision_history()).await;
    let client = drive.client().await;
    let (share, _, file) = open_fixture_file(&client).await;

    let revisions = client
        .revisions(share.volume_id(), file.id())
        .await
        .unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[1].state(), RevisionState::Obsolete);

    client
        .restore_revision(share.volume_id(), file.id(), revisions[1].id())
        .await
        .unwrap();
    client
        .delete_revision(share.volume_id(), file.id(), revisions[1].id())
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn saving_never_overwrites_or_leaves_partial_files() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let (share, _, file) = open_fixture_file(&client).await;
    let folder = tempfile::tempdir().unwrap();

    let (first, second) = tokio::join!(
        client
            .download(share.volume_id(), &file)
            .save_into(folder.path(), |_| {}),
        client
            .download(share.volume_id(), &file)
            .save_into(folder.path(), |_| {}),
    );

    let mut saved = [first.unwrap(), second.unwrap()];
    saved.sort();
    let numbered = FILE_NAME.replacen('.', " (1).", 1);
    assert_eq!(
        saved,
        [folder.path().join(numbered), folder.path().join(FILE_NAME)]
    );
    for path in &saved {
        assert_eq!(std::fs::read(path).unwrap(), CONTENT);
    }
    assert_eq!(std::fs::read_dir(folder.path()).unwrap().count(), 2);

    let corrupted = DummyDrive::with_shape(Shape::new().corrupted(Corruption::Block)).await;
    let corrupting = corrupted.client().await;
    let (share, _, file) = open_fixture_file(&corrupting).await;
    let untouched = tempfile::tempdir().unwrap();
    let refused = corrupting
        .download(share.volume_id(), &file)
        .save_into(untouched.path(), |_| {})
        .await;

    assert!(refused.is_err());
    assert_eq!(std::fs::read_dir(untouched.path()).unwrap().count(), 0);
}
