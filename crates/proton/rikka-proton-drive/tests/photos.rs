use jiff::Timestamp;
use rikka_proton_drive::{Error, LinkId, PhotoDetails, PhotoTag, VolumeId};
use rikka_proton_pgp::{LockedKey, unarmor};

use rikka_test_proton_drive::{
    ALBUM, CONTENT, COPIED, DUPLICATE, DummyDrive, ELSEWHERE, FILE, FILE_NAME, PHOTOS, SHOT,
    SHOT_BURST, SHOT_MOTION, Shape, THUMBNAIL, lookup_hash, sha1_hex,
};

const COMMIT: &str = "PUT /drive/v2/volumes/volume/files/*/revisions/*";
const COPY: &str = "POST /drive/volumes/elsewhere/links/shot/copy";

#[tokio::test(flavor = "multi_thread")]
async fn missing_photo_library_is_created() {
    let drive = DummyDrive::with_shape(Shape::new().photo_library_missing()).await;
    let client = drive.client().await;

    let library = client.photo_library().await.unwrap();
    assert_eq!(library.root().id().as_str(), "root");
    assert_eq!(library.volume_id().as_str(), "volume");

    let body = drive.last_body("POST /drive/photos/volumes");
    let share = &body["Share"];
    assert_eq!(
        share["AddressID"].as_str(),
        Some(rikka_test_proton::ADDRESS_ID)
    );
    assert_eq!(share["AddressKeyID"].as_str(), Some("address-key"));
    let share_passphrase = drive.decrypt_text(
        &drive.owner().address,
        share["Passphrase"].as_str().unwrap(),
    );
    let share_key = LockedKey::import(share["Key"].as_str().unwrap())
        .unwrap()
        .unlock(share_passphrase)
        .unwrap();

    let link = &body["Link"];
    let root_passphrase = drive.decrypt_text(&share_key, link["NodePassphrase"].as_str().unwrap());
    LockedKey::import(link["NodeKey"].as_str().unwrap())
        .unwrap()
        .unlock(root_passphrase)
        .unwrap();
    assert_eq!(
        drive.decrypt_text(&share_key, link["Name"].as_str().unwrap()),
        "root"
    );
    assert!(link["NodeHashKey"].as_str().is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn photo_upload_commits_metadata_and_thumbnails() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let library = client.photo_library().await.unwrap();
    let captured = Timestamp::from_second(1_650_000_000).unwrap();

    let photo = client
        .upload_photo(&library, "beach.jpg")
        .photo(
            PhotoDetails::new()
                .captured_at(captured)
                .main_photo("main")
                .tags([PhotoTag::LivePhotos, PhotoTag::Favorites]),
        )
        .thumbnail(THUMBNAIL.to_vec())
        .hd_thumbnail(b"a larger picture".to_vec())
        .upload_bytes(CONTENT.to_vec())
        .await
        .unwrap();
    assert!(photo.signatures().all_valid());

    let committed = drive.last_body(COMMIT);
    let committed_photo = &committed["Photo"];
    assert_eq!(committed_photo["CaptureTime"].as_i64(), Some(1_650_000_000));
    assert_eq!(
        committed_photo["ContentHash"].as_str(),
        Some(lookup_hash(drive.root_hash(), &sha1_hex(CONTENT)).as_str())
    );
    assert_eq!(committed_photo["MainPhotoLinkID"].as_str(), Some("main"));
    assert_eq!(committed_photo["Tags"], serde_json::json!([3, 0]));

    let kinds: Vec<u64> = drive.last_body("POST /drive/blocks")["ThumbnailList"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|listed| listed["Type"].as_u64())
        .collect();
    assert_eq!(kinds, [1, 2]);
}

#[tokio::test(flavor = "multi_thread")]
async fn timeline_pages_from_last_photo() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;

    let entries = client
        .timeline(&VolumeId::from("volume"))
        .page_size(2)
        .all()
        .await
        .unwrap();

    let ids: Vec<&str> = entries
        .iter()
        .map(|entry| entry.link_id().as_str())
        .collect();
    assert_eq!(ids, PHOTOS);
    assert_eq!(drive.request_count("GET /drive/volumes/volume/photos"), 3);
    assert_eq!(
        entries[0].tags(),
        [PhotoTag::Favorites, PhotoTag::LivePhotos]
    );
    assert_eq!(entries[0].related()[0].link_id().as_str(), "p1-motion");
    assert!(entries[0].captured_at() > entries[1].captured_at());
}

#[tokio::test(flavor = "multi_thread")]
async fn duplicates_match_name_and_content() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let library = client.photo_library().await.unwrap();

    let found = client
        .find_photo_duplicates(&library, "beach.jpg", &sha1_hex(CONTENT).to_uppercase())
        .await
        .unwrap();

    assert_eq!(found, [LinkId::from(DUPLICATE)]);
    assert_eq!(
        drive.last_body("POST /drive/volumes/volume/photos/duplicates")["NameHashes"][0].as_str(),
        Some(lookup_hash(drive.root_hash(), "beach.jpg").as_str())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn album_lifecycle_keeps_photos() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let library = client.photo_library().await.unwrap();
    let volume = library.volume_id().clone();

    let album = client.create_album(&library, "Summer").await.unwrap();
    assert!(album.is_album());
    assert_eq!(album.name(), "Summer");
    assert!(album.keys().hash_key().is_some());
    assert_eq!(
        drive.last_body("POST /drive/photos/volumes/volume/albums")["Link"]["Hash"].as_str(),
        Some(lookup_hash(drive.root_hash(), "Summer").as_str())
    );

    let photo = client
        .file(library.volume_id(), &LinkId::from(FILE), library.root())
        .await
        .unwrap();
    let added = client
        .add_to_album(
            &library,
            &album,
            library.root(),
            std::slice::from_ref(&photo),
        )
        .await
        .unwrap();
    assert!(added.iter().all(|outcome| outcome.is_ok()));

    let entry = &drive.last_body("POST /drive/photos/volumes/volume/albums/album/add-multiple")["AlbumData"]
        [0];
    assert_eq!(entry["LinkID"].as_str(), Some(FILE));
    assert_eq!(
        entry["Hash"].as_str().map(str::to_owned),
        album.hash(FILE_NAME).ok()
    );
    assert_eq!(
        entry["ContentHash"].as_str().map(str::to_owned),
        album.hash(&sha1_hex(CONTENT)).ok()
    );
    let name = drive
        .pgp()
        .decrypt()
        .with_key(album.keys().key().clone())
        .message(entry["Name"].as_str().unwrap())
        .unwrap();
    assert_eq!(name.data(), FILE_NAME.as_bytes());
    unarmor(entry["NodePassphrase"].as_str().unwrap()).unwrap();
    assert_eq!(
        drive.decrypt_text(
            album.keys().key(),
            entry["NodePassphrase"].as_str().unwrap()
        ),
        drive.file().passphrase
    );

    let removed = client
        .remove_from_album(&volume, album.id(), &[LinkId::from(FILE)])
        .await
        .unwrap();
    assert_eq!(removed.len(), 1);
    assert_eq!(
        drive.last_body("POST /drive/photos/volumes/volume/albums/album/remove-multiple")["LinkIDs"],
        serde_json::json!([FILE])
    );

    let refused = client
        .delete_album(&volume, &LinkId::from(ALBUM), false)
        .await
        .unwrap_err();
    assert!(
        matches!(&refused, Error::AlbumOutsideTimeline(children) if children == &[LinkId::from(FILE)]),
        "{refused}"
    );

    let albums = client.albums(&volume).await.unwrap();
    let ids: Vec<&str> = albums
        .iter()
        .map(|album| album.link_id().as_str())
        .collect();
    assert_eq!(ids, ["first", "second"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn album_rename_reuses_session_key() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let library = client.photo_library().await.unwrap();
    let volume = library.volume_id().clone();
    let album = client.create_album(&library, "Summer").await.unwrap();

    client
        .rename_album(&library, &album, "Winter")
        .await
        .unwrap();
    let link = &drive.last_body("PUT /drive/photos/volumes/volume/albums/album")["Link"];
    assert_eq!(
        link["Hash"].as_str(),
        Some(lookup_hash(drive.root_hash(), "Winter").as_str())
    );
    assert_eq!(
        link["OriginalHash"].as_str(),
        Some(lookup_hash(drive.root_hash(), "Summer").as_str())
    );

    let file = LinkId::from(FILE);
    client
        .tag_photo(&volume, &file, &[PhotoTag::Screenshots])
        .await
        .unwrap();
    let in_timeline = client.file(&volume, &file, library.root()).await.unwrap();
    client
        .favorite_photo(&library, library.root(), &in_timeline)
        .await
        .unwrap();
    client.unfavorite_photo(&volume, &file).await.unwrap();

    assert_eq!(
        drive.last_body("POST /drive/photos/volumes/volume/links/file/tags")["Tags"],
        serde_json::json!([1])
    );
    assert!(
        drive.last_body("POST /drive/photos/volumes/volume/links/file/favorite")["PhotoData"]
            .is_null()
    );
    assert_eq!(
        drive.last_body("DELETE /drive/photos/volumes/volume/links/file/tags")["Tags"],
        serde_json::json!([0])
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn cross_volume_copy_retries_with_related() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let library = client.photo_library().await.unwrap();
    let album = client.create_album(&library, "Summer").await.unwrap();
    let shot = client
        .file(
            &VolumeId::from(ELSEWHERE),
            &LinkId::from(SHOT),
            library.root(),
        )
        .await
        .unwrap();
    assert_eq!(
        shot.link().volume_id().map(|id| id.as_str()),
        Some(ELSEWHERE)
    );

    let added = client
        .add_to_album(
            &library,
            &album,
            library.root(),
            std::slice::from_ref(&shot),
        )
        .await
        .unwrap();
    assert_eq!(added.len(), 1);
    assert!(added[0].is_ok(), "{:?}", added[0].error());

    let copies = drive.bodies(COPY);
    let first = &copies[0];
    let related: Vec<&str> = first["Photos"]["RelatedPhotos"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|related| related["LinkID"].as_str())
        .collect();
    assert_eq!(related, [SHOT_MOTION]);
    let retried = &copies[1];
    let related: Vec<&str> = retried["Photos"]["RelatedPhotos"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|related| related["LinkID"].as_str())
        .collect();
    assert_eq!(related, [SHOT_BURST, SHOT_MOTION]);
    assert_eq!(retried["TargetVolumeID"].as_str(), Some("volume"));
    assert_eq!(retried["TargetParentLinkID"].as_str(), Some(ALBUM));
    assert_eq!(
        retried["Photos"]["ContentHash"].as_str().map(str::to_owned),
        album.hash(&sha1_hex(CONTENT)).ok()
    );
    assert!(retried["NodePassphraseSignature"].is_null());
    let name = drive
        .pgp()
        .decrypt()
        .with_key(album.keys().key().clone())
        .message(retried["Name"].as_str().unwrap())
        .unwrap();
    assert_eq!(name.data(), b"shot.jpg");

    let copied = client
        .copy_photo(&library, library.root(), library.root(), &shot)
        .await
        .unwrap();
    assert_eq!(copied.as_str(), COPIED);
}

#[tokio::test(flavor = "multi_thread")]
async fn save_to_timeline_moves_and_favorites() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let library = client.photo_library().await.unwrap();
    let photo = client
        .file(library.volume_id(), &LinkId::from(FILE), library.root())
        .await
        .unwrap();

    let saved = client
        .save_to_timeline(&library, library.root(), std::slice::from_ref(&photo))
        .await
        .unwrap();
    assert!(saved.iter().all(|outcome| outcome.is_ok()));
    let moved = drive.last_body("PUT /drive/photos/volumes/volume/links/transfer-multiple");
    assert_eq!(moved["ParentLinkID"].as_str(), Some("root"));
    assert_eq!(moved["Links"][0]["LinkID"].as_str(), Some(FILE));
    assert_eq!(
        moved["Links"][0]["OriginalHash"].as_str(),
        Some(lookup_hash(drive.root_hash(), FILE_NAME).as_str())
    );
    assert_eq!(
        moved["NameSignatureEmail"].as_str(),
        Some(rikka_test_proton::EMAIL)
    );

    let shot = client
        .file(
            &VolumeId::from(ELSEWHERE),
            &LinkId::from(SHOT),
            library.root(),
        )
        .await
        .unwrap();
    client
        .favorite_photo(&library, library.root(), &shot)
        .await
        .unwrap();
    let data =
        &drive.last_body("POST /drive/photos/volumes/elsewhere/links/shot/favorite")["PhotoData"];
    assert_eq!(
        data["Hash"].as_str(),
        Some(lookup_hash(drive.root_hash(), "shot.jpg").as_str())
    );
    assert_eq!(
        data["ContentHash"].as_str(),
        Some(lookup_hash(drive.root_hash(), &sha1_hex(CONTENT)).as_str())
    );
    assert_eq!(
        data["RelatedPhotos"][0]["LinkID"].as_str(),
        Some(SHOT_MOTION)
    );
}
