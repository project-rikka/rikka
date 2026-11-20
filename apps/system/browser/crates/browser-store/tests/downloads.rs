use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use browser_store::{DownloadProgress, DownloadState, NewDownload, Store};

fn new(name: &str) -> NewDownload {
    NewDownload {
        url: format!("https://files.example/{name}"),
        path: PathBuf::from(format!("/home/user/Downloads/{name}")),
        mime: "application/zip".to_owned(),
        total: Some(1000),
    }
}

#[test]
fn downloads_track_progress_and_completion() {
    let store = Store::in_memory().unwrap();
    let profile = store.create_profile("Default");
    let first = store.record_download(profile, new("a.zip"));
    let second = store.record_download(profile, new("b.zip"));
    for received in (0..=1000).step_by(100) {
        store.update_download(
            second,
            DownloadProgress {
                received,
                total: Some(1000),
                state: DownloadState::InProgress,
            },
        );
    }
    store.update_download(
        first,
        DownloadProgress {
            received: 1000,
            total: Some(1000),
            state: DownloadState::Finished,
        },
    );
    let renamed = PathBuf::from(OsStr::from_bytes(b"/home/user/Downloads/b\xff.zip"));
    store.set_download_path(second, renamed.clone());
    store.flush().unwrap();

    let downloads = store.downloads(profile, 10).unwrap();
    assert_eq!(downloads.len(), 2);
    let (b, a) = (&downloads[0], &downloads[1]);
    assert_eq!((a.id, b.id), (first, second));
    assert_eq!(b.received, 1000);
    assert_eq!(b.state, DownloadState::InProgress);
    assert_eq!(b.path, renamed);
    assert_eq!(b.finished_at, None);
    assert_eq!(a.state, DownloadState::Finished);
    assert!(a.finished_at.is_some());

    store.clear_finished_downloads(profile);
    store.flush().unwrap();
    let left: Vec<_> = store
        .downloads(profile, 10)
        .unwrap()
        .iter()
        .map(|d| d.id)
        .collect();
    assert_eq!(left, [second]);

    store.fail_interrupted_downloads();
    store.flush().unwrap();
    assert_eq!(
        store.downloads(profile, 10).unwrap()[0].state,
        DownloadState::Failed
    );

    store.remove_download(second);
    store.flush().unwrap();
    assert!(store.downloads(profile, 10).unwrap().is_empty());
}
