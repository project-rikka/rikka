use rikka_proton_cache::Cache;
use rikka_proton_drive::{Error, EventBatch, LinkId, Node, ShareId, VolumeId};
use rikka_proton_pgp::SymmetricKey;
use rikka_test_proton_drive::{DummyDrive, FILE_NAME, FOLDER_NAME, Shape, extra_folder_id};

const EXTRA_FOLDERS: usize = 200;

fn names(nodes: &[Node]) -> Vec<String> {
    nodes.iter().map(|node| node.name().to_owned()).collect()
}

fn expected_names() -> Vec<String> {
    [FOLDER_NAME.to_owned(), FILE_NAME.to_owned()]
        .into_iter()
        .chain((0..EXTRA_FOLDERS).map(extra_folder_id))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn large_folder_lists_in_api_order() {
    let drive = DummyDrive::with_shape(Shape::new().extra_folders(EXTRA_FOLDERS)).await;
    let client = drive.client().await;
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();

    let children = client.children(share.volume_id(), &root).await.unwrap();

    assert_eq!(names(&children), expected_names());
    assert!(children.iter().all(|node| node.signatures().all_valid()));
    assert_eq!(
        drive.request_count("GET /drive/v2/volumes/volume/folders/root/children"),
        2
    );

    let opened_root = client
        .open_root(
            &VolumeId::from("volume"),
            &ShareId::from("share"),
            &LinkId::from("root"),
        )
        .await
        .unwrap();
    assert_eq!(names(opened_root.children()), expected_names());
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_root_missing_from_share() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;

    let refused = client
        .open_root(
            &VolumeId::from("volume"),
            &ShareId::from("share"),
            &LinkId::from("folder"),
        )
        .await
        .unwrap_err();

    assert!(matches!(refused, Error::MovedRoot(_)), "{refused}");
}

#[tokio::test(flavor = "multi_thread")]
async fn listing_cached_until_event() {
    let drive = DummyDrive::with_shape(Shape::new().extra_folders(EXTRA_FOLDERS)).await;
    let client = drive.client().await;
    let cache = Cache::in_memory(SymmetricKey::generate()).unwrap();
    let client = client.with_cache(cache.clone());
    let share = client.open_share(&ShareId::from("share")).await.unwrap();
    let root = client.root(&share).await.unwrap();
    assert!(client.cached_children(&root).await.unwrap().is_none());

    client.children(share.volume_id(), &root).await.unwrap();
    cache.flush().unwrap();
    let request = drive.request_count("GET /drive/v2/volumes/volume/folders/root/children");

    let cached = client.cached_children(&root).await.unwrap().unwrap();
    assert_eq!(names(&cached), expected_names());
    assert!(cached.iter().all(|node| node.signatures().all_valid()));
    assert_eq!(
        drive.request_count("GET /drive/v2/volumes/volume/folders/root/children"),
        request
    );

    let created: EventBatch = serde_json::from_value(serde_json::json!({
        "EventID": "event-2",
        "Events": [{
            "EventID": "event-2",
            "EventType": 1,
            "Link": {"LinkID": "newcomer", "ParentLinkID": "root"}
        }]
    }))
    .unwrap();
    client.apply_events(&created);
    assert!(client.cached_children(&root).await.unwrap().is_none());
}
