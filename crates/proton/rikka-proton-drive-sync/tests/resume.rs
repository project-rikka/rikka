use rikka_proton_drive::BLOCK_SIZE;
use rikka_proton_drive_sync::{Direction, ItemState};
use rikka_test_proton_drive::DummyDrive;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::oneshot;

use crate::support::{Remote, computer, sync_now};

const BLOCKS_BEFORE_CRASH: usize = 4;

#[tokio::test(flavor = "multi_thread")]
async fn interrupted_upload_resumes_from_draft() {
    let drive = DummyDrive::start().await;
    let home = tempfile::tempdir().unwrap();
    let folder = home.path().join("Large");
    let state = home.path().join("state");
    std::fs::create_dir_all(&folder).unwrap();
    let content: Vec<u8> = (0..BLOCK_SIZE * BLOCKS_BEFORE_CRASH + 999)
        .map(|offset| (offset % 253) as u8)
        .collect();
    std::fs::write(folder.join("video.bin"), &content).unwrap();

    drive.stall_uploads_after(BLOCKS_BEFORE_CRASH);
    let crashing = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let (crash_drive, crash_folder, crash_state) = (drive.clone(), folder.clone(), state.clone());
    let (journaled, journaled_receiver) = oneshot::channel();
    crashing.spawn(async move {
        let sync = computer(&crash_drive, &crash_folder, &crash_state).await;
        let mut items = sync.items();
        let journaled_bytes = (BLOCK_SIZE * BLOCKS_BEFORE_CRASH) as u64;
        loop {
            match items.recv().await {
                Ok(update)
                    if matches!(
                        update.state(),
                        ItemState::Transferred { direction: Direction::Up, bytes }
                            if *bytes >= journaled_bytes
                    ) =>
                {
                    break;
                }
                Ok(_) | Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => panic!("sync stopped before journaling the upload"),
            }
        }
        journaled.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    tokio::join!(
        drive.wait_for_block_uploads(BLOCKS_BEFORE_CRASH + 1),
        async { journaled_receiver.await.unwrap() },
    );
    crashing.shutdown_background();
    drive.release_uploads();

    let before = drive.block_uploads();
    let sync = computer(&drive, &folder, &state).await;
    sync_now(&sync).await;
    let device = sync.device().unwrap().clone();
    let remote = Remote::device_folder(&drive, &device, "Large").await;
    assert_eq!(remote.read("video.bin").await, content);
    assert_eq!(drive.block_uploads() - before, 1);
    assert_eq!(
        drive.request_count("POST /drive/v2/volumes/volume/files"),
        1
    );
    sync.shutdown().await.unwrap();
}
