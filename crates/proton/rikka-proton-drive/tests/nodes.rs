use rikka_proton_drive::{DocumentType, Error, Node, ShareId, VolumeId};
use rikka_test_proton_drive::{Corruption, DummyDrive, FILE_NAME, FOLDER_NAME, Shape, lookup_hash};

const CHECKED: &str = "POST /drive/v2/volumes/volume/links/root/checkAvailableHashes";

#[tokio::test(flavor = "multi_thread")]
async fn root_children_open_with_node_key() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();

    let children = client.children(share.volume_id(), &root).await.unwrap();
    assert_eq!(children.len(), 2);

    let folder = children[0].as_folder().unwrap();
    assert_eq!(folder.name(), FOLDER_NAME);
    assert_eq!(
        folder.link().hash(),
        Some(lookup_hash(drive.root_hash(), FOLDER_NAME).as_str())
    );
    assert!(folder.keys().hash_key().is_some());
    assert!(folder.signatures().all_valid());

    let file = children[1].as_file().unwrap();
    assert_eq!(file.name(), FILE_NAME);
    assert_eq!(file.media_type(), "text/plain");
    assert_eq!(
        file.content_key().unwrap().as_bytes(),
        drive.content().as_bytes()
    );
    assert_eq!(
        file.attributes()
            .and_then(rikka_proton_drive::ExtendedAttributes::size),
        Some(41)
    );
    assert_eq!(file.revision_id().unwrap().as_str(), "revision");
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_mismatched_passphrase_signature() {
    let drive = DummyDrive::with_shape(Shape::new().corrupted(Corruption::Passphrase)).await;
    let client = drive.client().await;
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();

    let refused = client.children(share.volume_id(), &root).await.unwrap_err();

    assert!(
        matches!(refused, Error::Signature { field, .. } if field == "node passphrase"),
        "{refused}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn created_folder_round_trips_name_and_hash() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();

    let made = client.create_folder(&share, &root, "trips").await.unwrap();

    assert_eq!(made.name(), "trips");
    assert!(made.signatures().all_valid());
    assert!(made.keys().hash_key().is_some());

    let body = drive.last_body("POST /drive/v2/volumes/volume/folders");
    assert_eq!(
        body["Hash"].as_str(),
        Some(lookup_hash(drive.root_hash(), "trips").as_str())
    );
    assert_eq!(body["ParentLinkID"].as_str(), Some("root"));
    assert_eq!(
        body["SignatureEmail"].as_str(),
        Some(rikka_test_proton::EMAIL)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rename_and_move_send_name_hashes() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();
    let children = client.children(share.volume_id(), &root).await.unwrap();

    let Node::File(file) = &children[1] else {
        panic!("expected file as second child");
    };
    let folder = children[0].as_folder().unwrap();
    let node = Node::File(file.clone());

    client
        .rename(&share, &root, &node, "postcard.txt")
        .await
        .unwrap();
    let renamed = drive.last_body("PUT /drive/v2/volumes/volume/links/file/rename");
    assert_eq!(
        renamed["Hash"].as_str(),
        Some(lookup_hash(drive.root_hash(), "postcard.txt").as_str())
    );
    assert_eq!(
        renamed["OriginalHash"].as_str(),
        Some(lookup_hash(drive.root_hash(), FILE_NAME).as_str())
    );
    assert_eq!(
        renamed["NameSignatureEmail"].as_str(),
        Some(rikka_test_proton::EMAIL)
    );

    client.move_node(&share, &node, folder).await.unwrap();
    let moved = drive.last_body("PUT /drive/v2/volumes/volume/links/file/move");
    assert_eq!(moved["ParentLinkID"].as_str(), Some("folder"));
    assert_eq!(
        moved["NameSignatureEmail"].as_str(),
        Some(rikka_test_proton::EMAIL)
    );
    assert!(moved["NodePassphrase"].as_str().is_some());
    assert!(
        moved["NodePassphraseSignature"].is_null(),
        "node lost its passphrase signature"
    );
    assert!(
        moved["ContentHash"].is_null(),
        "non-photo has a content hash"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn available_name_picks_first_free_number() {
    let drive =
        DummyDrive::with_shape(Shape::new().extra_files(["holiday (1).txt", "holiday (2).txt"]))
            .await;
    let client = drive.client().await;
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();

    let free = client
        .available_name(share.volume_id(), &root, FILE_NAME)
        .await
        .unwrap();

    assert_eq!(free, "holiday (3).txt");
    let body = drive.last_body(CHECKED);
    let hashes = body["Hashes"].as_array().unwrap();
    assert_eq!(hashes.len(), 11);
    assert_eq!(
        hashes[0].as_str(),
        Some(lookup_hash(drive.root_hash(), FILE_NAME).as_str())
    );
    assert_eq!(
        hashes[1].as_str(),
        Some(lookup_hash(drive.root_hash(), "holiday (1).txt").as_str())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn creates_signed_proton_document() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();

    let document = client
        .create_document(&share, &root, "Notes", DocumentType::Document)
        .await
        .unwrap();

    assert_eq!(document.name(), "Notes");
    assert!(document.is_proton_document());
    assert_eq!(
        DocumentType::from_file(&document),
        Some(DocumentType::Document)
    );
    assert_eq!(
        document.document_url(&VolumeId::from("volume")).unwrap(),
        "https://docs.proton.me/doc?mode=open&volumeId=volume&linkId=document"
    );

    let body = drive.last_body("POST /drive/v2/volumes/volume/documents");
    assert_eq!(body["DocumentType"].as_i64(), Some(1));
    assert_eq!(
        body["Hash"].as_str(),
        Some(lookup_hash(drive.root_hash(), "Notes").as_str())
    );
    let manifest = body["ManifestSignature"].as_str().unwrap();
    assert!(
        drive
            .pgp()
            .verify()
            .with_key(drive.owner().address.public_key())
            .detached(b"", manifest)
            .is_valid()
    );

    let file = client
        .file(
            share.volume_id(),
            &rikka_proton_drive::LinkId::from("file"),
            &root,
        )
        .await
        .unwrap();
    assert!(matches!(
        file.document_url(&VolumeId::from("volume")),
        Err(Error::NotADocument(_))
    ));
}
