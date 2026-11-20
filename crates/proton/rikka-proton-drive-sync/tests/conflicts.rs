use rikka_test_proton_drive::DummyDrive;

use crate::support::{DEVICE, Remote, computer, names, sync_now};

#[tokio::test(flavor = "multi_thread")]
async fn edit_conflict_keeps_both_versions() {
    let drive = DummyDrive::start().await;
    let home = tempfile::tempdir().unwrap();
    let folder = home.path().join("Notes");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("report.txt"), b"the shared start").unwrap();

    let sync = computer(&drive, &folder, &home.path().join("state")).await;
    sync_now(&sync).await;
    let device = sync.device().unwrap().clone();
    let remote = Remote::device_folder(&drive, &device, "Notes").await;

    sync.pause();
    std::fs::write(folder.join("report.txt"), b"written on this machine").unwrap();
    remote.revise("report.txt", b"written elsewhere").await;
    sync_now(&sync).await;
    sync.resume();
    sync_now(&sync).await;

    assert_eq!(
        std::fs::read(folder.join("report.txt")).unwrap(),
        b"written elsewhere"
    );
    let listed = names(&folder);
    assert_eq!(listed.len(), 2, "{listed:?}");
    let copy = listed
        .iter()
        .find(|name| name.as_str() != "report.txt")
        .unwrap();
    assert!(
        copy.starts_with("report (# Edit conflict ")
            && copy.ends_with(&format!(" {DEVICE} #).txt")),
        "{copy}"
    );
    assert_eq!(
        std::fs::read(folder.join(copy)).unwrap(),
        b"written on this machine"
    );
    assert_eq!(sync.status().conflicts(), [folder.join(copy)]);

    let mut expected = vec![copy.clone(), "report.txt".to_owned()];
    expected.sort();
    assert_eq!(remote.names().await, expected);
    assert_eq!(remote.read(copy).await, b"written on this machine");
    sync.shutdown().await.unwrap();
}
