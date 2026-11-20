use jiff::Timestamp;
use rikka_proton_drive::DeviceSync;
use rikka_proton_drive_sync::{Activity, DriveSync};
use rikka_test_proton_drive::DummyDrive;

use crate::support::{Remote, computer, names, sync_now};

const UPLOADS: &str = "POST /drive/v2/volumes/volume/files";

#[tokio::test(flavor = "multi_thread")]
async fn computer_folder_syncs_across_restart() {
    let drive = DummyDrive::start().await;
    let home = tempfile::tempdir().unwrap();
    let folder = home.path().join("Documents");
    let state = home.path().join("state");
    std::fs::create_dir_all(folder.join("photos")).unwrap();
    std::fs::write(folder.join("notes.txt"), b"the first draft").unwrap();
    std::fs::write(folder.join("photos").join("cat.jpg"), b"a cat").unwrap();
    std::fs::write(folder.join("~$notes.docx"), b"a lock file").unwrap();

    let started = Timestamp::now().as_second();
    let sync = computer(&drive, &folder, &state).await;
    sync_now(&sync).await;
    assert_eq!(sync.status().activity(), Activity::Idle);
    let device = sync.device().unwrap().clone();
    let listed = drive.client().await.device(&device).await.unwrap();
    assert_eq!(listed.sync(), DeviceSync::On);
    assert!(
        listed
            .last_synced_at()
            .is_some_and(|at| at.as_second() >= started),
        "{:?}",
        listed.last_synced_at()
    );
    let remote = Remote::device_folder(&drive, &device, "Documents").await;
    assert_eq!(remote.names().await, ["notes.txt", "photos"]);
    assert_eq!(remote.read("notes.txt").await, b"the first draft");
    assert_eq!(
        remote.folder("photos").await.read("cat.jpg").await,
        b"a cat"
    );

    remote
        .upload("from-phone.txt", b"written on the phone")
        .await;
    sync_now(&sync).await;
    assert_eq!(
        std::fs::read(folder.join("from-phone.txt")).unwrap(),
        b"written on the phone"
    );

    remote
        .revise("from-phone.txt", b"edited on the phone")
        .await;
    std::fs::rename(folder.join("notes.txt"), folder.join("renamed.txt")).unwrap();
    sync_now(&sync).await;
    assert_eq!(
        std::fs::read(folder.join("from-phone.txt")).unwrap(),
        b"edited on the phone"
    );
    assert_eq!(
        remote.names().await,
        ["from-phone.txt", "photos", "renamed.txt"]
    );
    assert_eq!(remote.read("renamed.txt").await, b"the first draft");

    std::fs::remove_file(folder.join("photos").join("cat.jpg")).unwrap();
    remote.trash("from-phone.txt").await;
    sync_now(&sync).await;
    assert!(remote.folder("photos").await.names().await.is_empty());
    assert_eq!(names(&folder), ["photos", "renamed.txt", "~$notes.docx"]);

    let uploads = drive.request_count(UPLOADS);
    sync.shutdown().await.unwrap();
    std::fs::write(folder.join("later.txt"), b"written while stopped").unwrap();
    let sync = computer(&drive, &folder, &state).await;
    sync_now(&sync).await;
    assert_eq!(sync.device(), Some(&device));
    assert_eq!(drive.request_count("POST /drive/devices"), 1);
    assert_eq!(drive.request_count(UPLOADS), uploads + 1);
    assert_eq!(remote.names().await, ["later.txt", "photos", "renamed.txt"]);
    sync.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn folder_maps_into_my_files() {
    let drive = DummyDrive::start().await;
    let home = tempfile::tempdir().unwrap();
    let folder = home.path().join("Work");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("plan.md"), b"# Plan").unwrap();

    let sync = DriveSync::builder(drive.client().await)
        .state_folder(home.path().join("state"))
        .my_files_folder(&folder, "Work/Projects")
        .start()
        .await
        .unwrap();
    sync_now(&sync).await;
    assert_eq!(sync.device(), None);
    assert_eq!(drive.request_count("POST /drive/devices"), 0);

    let remote = Remote::my_files(&drive, &["Work", "Projects"]).await;
    assert_eq!(remote.read("plan.md").await, b"# Plan");
    sync.shutdown().await.unwrap();
}
