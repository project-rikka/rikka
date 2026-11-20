use std::ffi::OsStr;
use std::time::Duration;

use rikka_proton_drive_sync::{DriveSync, ItemState, VolumeFeed};
use rikka_test_proton_drive::DummyDrive;

use crate::support::{DEVICE, Remote, sync_now};

const FEED_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test(flavor = "multi_thread")]
async fn volume_feed_delivers_remote_changes() {
    let drive = DummyDrive::start().await;
    let home = tempfile::tempdir().unwrap();
    let folder = home.path().join("Documents");
    std::fs::create_dir_all(&folder).unwrap();
    let feed = VolumeFeed::new();
    let sync = DriveSync::builder(drive.client().await)
        .state_folder(home.path().join("state"))
        .device_name(DEVICE)
        .computer_folder(&folder)
        .with_feed(&feed)
        .start()
        .await
        .unwrap();
    sync_now(&sync).await;
    let device = sync.device().unwrap().clone();
    let remote = Remote::device_folder(&drive, &device, "Documents").await;
    let since = remote.client().latest_event(remote.volume()).await.unwrap();

    remote
        .upload("from-phone.txt", b"written on the phone")
        .await;
    let batch = remote
        .client()
        .events(remote.volume(), &since)
        .await
        .unwrap();
    let mut items = sync.items();
    feed.publish(remote.volume(), &since, &batch);

    let arrived = folder.join("from-phone.txt");
    tokio::time::timeout(FEED_TIMEOUT, async {
        loop {
            let update = items.recv().await.unwrap();
            if update.path().file_name() == Some(OsStr::new("from-phone.txt"))
                && *update.state() == ItemState::Synced
            {
                break;
            }
        }
    })
    .await
    .unwrap();
    sync.shutdown().await.unwrap();
    assert_eq!(std::fs::read(&arrived).unwrap(), b"written on the phone");
}
