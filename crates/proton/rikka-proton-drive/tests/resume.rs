use std::sync::{Arc, Mutex};

use rikka_proton_drive::{BLOCK_SIZE, ResumePoint, UploadProgress};
use rikka_test_proton_drive::DummyDrive;

const SENT_FIRST: usize = 4;
const STALLED: usize = 2;

#[tokio::test(flavor = "multi_thread")]
async fn resume_uploads_only_missing_blocks() {
    let drive = DummyDrive::start().await;
    let client = drive.client().await;
    let volume = client.volumes().await.unwrap().into_iter().next().unwrap();
    let share = client.open_share(volume.share().id()).await.unwrap();
    let root = client.root(&share).await.unwrap();
    let content: Vec<u8> = (0..BLOCK_SIZE * 5 + 777)
        .map(|offset| (offset % 251) as u8)
        .collect();

    drive.stall_uploads_after(SENT_FIRST);
    let reported = Arc::new(Mutex::new(Vec::new()));
    let recorder = Arc::clone(&reported);
    let interrupted = client
        .upload(&share, &root, "large.bin")
        .on_progress(move |progress| recorder.lock().unwrap().push(progress))
        .upload_bytes(content.clone());
    tokio::select! {
        finished = interrupted => panic!("upload finished while stalled: {finished:?}"),
        () = drive.wait_for_block_uploads(SENT_FIRST + STALLED) => {}
    }
    drive.release_uploads();

    let reported = reported.lock().unwrap().clone();
    let Some(UploadProgress::DraftCreated { link, revision }) = reported.first().cloned() else {
        panic!("expected draft report first: {reported:?}");
    };
    let sent: Vec<_> = reported
        .iter()
        .filter_map(|progress| match progress {
            UploadProgress::BlockUploaded(block) => Some(*block),
            UploadProgress::DraftCreated { .. } => None,
        })
        .collect();
    assert_eq!(
        sent.iter().map(|block| block.index()).collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );

    let before = drive.block_uploads();
    let file = client
        .upload(&share, &root, "large.bin")
        .resume_from(ResumePoint::new(link.clone(), revision.clone(), sent))
        .upload_bytes(content.clone())
        .await
        .unwrap();
    assert_eq!(file.id(), &link);
    assert_eq!(file.revision_id().unwrap(), &revision);
    assert_eq!(drive.block_uploads() - before, STALLED);
    assert_eq!(
        drive.request_count("POST /drive/v2/volumes/volume/files"),
        1
    );
    assert_eq!(
        client
            .download(volume.id(), &file)
            .into_vec()
            .await
            .unwrap(),
        content
    );
}
