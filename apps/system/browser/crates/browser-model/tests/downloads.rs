use std::path::{Path, PathBuf};

use browser_model::{DownloadKey, DownloadPhase, Downloads, ProfileKey, StartedDownload};
use rikka_web::{Download, DownloadId, DownloadState};

fn begun(downloads: &mut Downloads, profile: ProfileKey, path: &Path) -> DownloadKey {
    let key = downloads.begin(StartedDownload {
        profile,
        url: "https://example.com/report.pdf".to_owned(),
        name: "report.pdf".to_owned(),
        mime: "application/pdf".to_owned(),
        total: Some(100),
        stored: None,
    });
    downloads.chosen(key, path);
    key
}

fn progress(id: &str, path: &Path, received: u64, state: DownloadState) -> Download {
    Download {
        id: DownloadId(id.to_owned()),
        uri: "https://example.com/report.pdf".to_owned(),
        referrer: None,
        private: true,
        path: path.to_owned(),
        mime_type: None,
        state,
        received,
        total: Some(100),
        started: None,
        error: None,
    }
}

#[test]
fn progress_finds_its_item_by_path_then_by_engine_id() {
    let mut downloads = Downloads::default();
    let chosen = PathBuf::from("/downloads/report.pdf");
    let renamed = PathBuf::from("/downloads/report (2).pdf");
    let key = begun(&mut downloads, ProfileKey::Private, &chosen);

    let first = downloads
        .update_progress(&progress("g1", &chosen, 40, DownloadState::Downloading))
        .expect("matched by the chosen path");
    assert_eq!(first.item, key);
    assert!(!first.finished);

    let last = downloads
        .update_progress(&progress("g1", &renamed, 100, DownloadState::Finished))
        .expect("matched by the engine id after the path moved");
    assert_eq!(last.item, key);
    assert!(last.finished);
    let item = downloads.item(key).expect("item");
    assert_eq!(item.state, DownloadPhase::Finished);
    assert_eq!(item.name, "report (2).pdf");
    assert!(
        downloads
            .update_progress(&progress("other", &chosen, 1, DownloadState::Downloading))
            .is_none()
    );
}

#[test]
fn clearing_keeps_running_downloads_and_other_profiles() {
    let mut downloads = Downloads::default();
    let done = PathBuf::from("/downloads/done.bin");
    let running = PathBuf::from("/downloads/running.bin");
    let finished = begun(&mut downloads, ProfileKey::Private, &done);
    let active = begun(&mut downloads, ProfileKey::Private, &running);
    downloads.update_progress(&progress("a", &done, 100, DownloadState::Finished));
    downloads.update_progress(&progress("b", &running, 50, DownloadState::Downloading));

    let cleared: Vec<DownloadKey> = downloads
        .clear_ended(ProfileKey::Private)
        .into_iter()
        .map(|item| item.key)
        .collect();
    assert_eq!(cleared, [finished]);
    assert!(downloads.item(active).is_some());
    assert!(downloads.claims(&running));
    let activity = downloads.activity(ProfileKey::Private);
    assert_eq!(activity.running, 1);
    assert_eq!(activity.fraction, Some(0.5));
}
