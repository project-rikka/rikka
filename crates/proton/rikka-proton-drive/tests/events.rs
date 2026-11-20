use std::sync::{Arc, Mutex};

use rikka_proton_account::{Account, Cursors, Feed, Poll};
use rikka_proton_drive::{DriveFeed, EventAction, EventId, LinkId, ShareId, VolumeId};
use rikka_test_proton_drive::{DummyDrive, FILE, FOLDER, SHARE, Shape, VOLUME};

#[tokio::test(flavor = "multi_thread")]
async fn catch_up_follows_more_pages() {
    let drive =
        DummyDrive::with_shape(Shape::new().events_page(2).convertible_invitation(FOLDER)).await;
    let client = drive.client().await;
    let volume = VolumeId::from(VOLUME);
    let share = client.open_share(&ShareId::from(SHARE)).await.unwrap();
    let root = client.root(&share).await.unwrap();
    let children = client.children(&volume, &root).await.unwrap();
    let folder = children[0].clone();
    let since = client.latest_event(&volume).await.unwrap();

    client
        .move_node(&share, &children[1], folder.as_folder().unwrap())
        .await
        .unwrap();
    client.share_node(&share, &folder).await.unwrap();
    client
        .trash(&volume, &[LinkId::from(FOLDER)])
        .await
        .unwrap();
    client
        .delete(&volume, &[LinkId::from(FOLDER)])
        .await
        .unwrap();
    let log = client.catch_up(&volume, &since).await.unwrap();

    assert_eq!(log.last().as_str(), "event-4");
    assert!(!log.needs_refresh());
    assert_eq!(
        drive.request_count("GET /drive/v2/volumes/volume/events/event-2"),
        1
    );
    assert_eq!(
        log.convertible_invitations()
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        [FOLDER]
    );

    let events = log.events();
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].action(), EventAction::UpdateMetadata);
    assert_eq!(events[0].link().id().as_str(), FILE);
    assert_eq!(
        events[0].link().parent_id().map(|id| id.as_str()),
        Some(FOLDER)
    );
    assert!(events[1].link().is_shared());
    assert_eq!(events[2].action(), EventAction::Update);
    assert!(events[2].link().is_trashed());
    assert!(events[2].link().is_shared());
    assert_eq!(events[3].action(), EventAction::Delete);
    assert_eq!(events[3].link().id().as_str(), FOLDER);
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_drops_events_returns_anchor() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let volume = VolumeId::from(VOLUME);

    let log = client
        .catch_up(&volume, &EventId::from("stale"))
        .await
        .unwrap();

    assert!(log.needs_refresh());
    assert!(log.events().is_empty());
    assert_eq!(log.last(), &client.latest_event(&volume).await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn events_start_at_latest_anchor() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let volume = VolumeId::from(VOLUME);

    let since = client.latest_event(&volume).await.unwrap();
    client.trash(&volume, &[LinkId::from(FILE)]).await.unwrap();
    let batch = client.events(&volume, &since).await.unwrap();

    assert_eq!(batch.id().as_str(), "event-1");
    assert!(!batch.has_more());
    assert_eq!(batch.events()[0].action(), EventAction::Update);
}

#[tokio::test(flavor = "multi_thread")]
async fn feed_reports_changes_after_anchoring() {
    let drive = DummyDrive::with_shape(Shape::new()).await;
    let client = drive.client().await;
    let volume = VolumeId::from(VOLUME);
    let keys = Arc::new(client.keys().clone());
    let cursors = Cursors::for_account(&Account::new(client.api().clone()));
    let anchor = client.latest_event(&volume).await.unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let feed = DriveFeed::new(client.clone()).observe({
        let observed = Arc::clone(&observed);
        move |volume, since, batch| {
            observed
                .lock()
                .unwrap()
                .push((volume.clone(), since.clone(), batch.id().clone()));
        }
    });

    let anchoring = Poll::new(&keys, &cursors);
    feed.poll(&anchoring).await.unwrap();
    assert!(anchoring.into_changes().is_empty());

    client.trash(&volume, &[LinkId::from(FILE)]).await.unwrap();
    let following = Poll::new(&keys, &cursors);
    feed.poll(&following).await.unwrap();
    let changes = following.into_changes();

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].volume(), &volume);
    assert_eq!(
        changes[0].events().events()[0].action(),
        EventAction::Update
    );
    assert_eq!(
        *observed.lock().unwrap(),
        [(volume, anchor, changes[0].events().id().clone())]
    );
}
