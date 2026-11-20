use std::collections::HashSet;
use std::path::{Path, PathBuf};

use browser_store::{Download, DownloadId, DownloadState, Timestamp};
use rikka_ui::Revision;
use rikka_web::DownloadId as WebId;

use crate::{ProfileId, ProfileKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DownloadKey {
    StoredDownload(DownloadId),
    Private(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DownloadPhase {
    Choosing,
    Running,
    Paused,
    Finished,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DownloadItem {
    pub key: DownloadKey,
    pub profile: ProfileKey,
    pub web: Option<WebId>,
    pub url: String,
    pub path: Option<PathBuf>,
    pub name: String,
    pub mime: String,
    pub total: Option<u64>,
    pub received: u64,
    pub state: DownloadPhase,
    pub started: Timestamp,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartedDownload {
    pub profile: ProfileKey,
    pub url: String,
    pub name: String,
    pub mime: String,
    pub total: Option<u64>,
    pub stored: Option<DownloadId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredDownload {
    pub id: DownloadId,
    pub progress: browser_store::DownloadProgress,
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgressUpdate {
    pub item: DownloadKey,
    pub stored: Option<StoredDownload>,
    pub finished: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DownloadActivity {
    pub running: usize,
    pub fraction: Option<f32>,
}

#[derive(Debug, Default)]
pub struct Downloads {
    items: Vec<DownloadItem>,
    loaded: HashSet<ProfileId>,
    private: u64,
    revision: Revision,
}

impl DownloadPhase {
    fn stored(self) -> DownloadState {
        match self {
            DownloadPhase::Choosing | DownloadPhase::Running | DownloadPhase::Paused => {
                DownloadState::InProgress
            }
            DownloadPhase::Finished => DownloadState::Finished,
            DownloadPhase::Failed => DownloadState::Failed,
            DownloadPhase::Cancelled => DownloadState::Cancelled,
        }
    }

    fn from_stored(state: DownloadState) -> DownloadPhase {
        match state {
            DownloadState::Finished => DownloadPhase::Finished,
            DownloadState::Cancelled => DownloadPhase::Cancelled,
            DownloadState::InProgress | DownloadState::Failed => DownloadPhase::Failed,
        }
    }

    fn from_web(state: rikka_web::DownloadState) -> DownloadPhase {
        match state {
            rikka_web::DownloadState::Downloading => DownloadPhase::Running,
            rikka_web::DownloadState::Paused => DownloadPhase::Paused,
            rikka_web::DownloadState::Finished => DownloadPhase::Finished,
            rikka_web::DownloadState::Failed => DownloadPhase::Failed,
            rikka_web::DownloadState::Cancelled => DownloadPhase::Cancelled,
        }
    }

    pub fn ended(self) -> bool {
        matches!(
            self,
            DownloadPhase::Finished | DownloadPhase::Failed | DownloadPhase::Cancelled
        )
    }
}

impl DownloadItem {
    fn from_stored(download: Download) -> DownloadItem {
        let name = download_name(&download.path, &download.url);
        DownloadItem {
            key: DownloadKey::StoredDownload(download.id),
            profile: ProfileKey::Persistent(download.profile),
            web: None,
            name,
            url: download.url,
            path: Some(download.path).filter(|path| !path.as_os_str().is_empty()),
            mime: download.mime,
            total: download.total,
            received: download.received,
            state: DownloadPhase::from_stored(download.state),
            started: download.started_at,
        }
    }

    pub fn fraction(&self) -> Option<f32> {
        let total = self.total.filter(|total| *total > 0)?;
        Some((self.received as f64 / total as f64).clamp(0.0, 1.0) as f32)
    }
}

pub fn download_name(path: &Path, url: &str) -> String {
    if let Some(name) = path.file_name() {
        return name.to_string_lossy().into_owned();
    }
    url::Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_owned))
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| url.to_owned())
}

impl Downloads {
    pub fn items(&self, profile: ProfileKey) -> impl Iterator<Item = &DownloadItem> {
        self.revision.read();
        self.items
            .iter()
            .filter(move |item| item.profile == profile)
    }

    pub fn item(&self, key: DownloadKey) -> Option<&DownloadItem> {
        self.revision.read();
        self.items.iter().find(|item| item.key == key)
    }

    pub fn claims(&self, path: &Path) -> bool {
        self.items
            .iter()
            .any(|item| !item.state.ended() && item.path.as_deref() == Some(path))
    }

    pub fn activity(&self, profile: ProfileKey) -> DownloadActivity {
        let running: Vec<&DownloadItem> = self
            .items(profile)
            .filter(|item| !item.state.ended())
            .collect();
        let known = running.iter().all(|item| item.fraction().is_some());
        let (received, total) = running
            .iter()
            .fold((0u64, 0u64), |(received, total), item| {
                (received + item.received, total + item.total.unwrap_or(0))
            });
        DownloadActivity {
            running: running.len(),
            fraction: (known && total > 0)
                .then(|| (received as f64 / total as f64).clamp(0.0, 1.0) as f32),
        }
    }

    pub fn needs_loading(&mut self, profile: ProfileId) -> bool {
        self.loaded.insert(profile)
    }

    pub fn begin(&mut self, begun: StartedDownload) -> DownloadKey {
        let key = match begun.stored {
            Some(id) => DownloadKey::StoredDownload(id),
            None => {
                self.private += 1;
                DownloadKey::Private(self.private)
            }
        };
        self.items.insert(
            0,
            DownloadItem {
                key,
                profile: begun.profile,
                web: None,
                url: begun.url,
                path: None,
                name: begun.name,
                mime: begun.mime,
                total: begun.total,
                received: 0,
                state: DownloadPhase::Choosing,
                started: Timestamp::now(),
            },
        );
        self.revision.changed();
        key
    }

    pub fn chosen(&mut self, key: DownloadKey, path: &Path) -> Option<StoredDownload> {
        let item = self.items.iter_mut().find(|item| item.key == key)?;
        item.state = DownloadPhase::Running;
        item.name = download_name(path, &item.url);
        item.path = Some(path.to_owned());
        self.revision.changed();
        stored(item, true)
    }

    pub fn update_progress(&mut self, download: &rikka_web::Download) -> Option<ProgressUpdate> {
        let item = self.items.iter_mut().find(|item| {
            item.web.as_ref() == Some(&download.id)
                || (item.web.is_none()
                    && !item.state.ended()
                    && item.path.as_deref() == Some(download.path.as_path()))
        })?;
        item.web = Some(download.id.clone());
        let moved = item.path.as_deref() != Some(download.path.as_path());
        item.path = Some(download.path.clone());
        item.name = download_name(&download.path, &item.url);
        item.received = download.received;
        if download.total.is_some() {
            item.total = download.total;
        }
        let state = DownloadPhase::from_web(download.state);
        let finished = state == DownloadPhase::Finished && item.state != DownloadPhase::Finished;
        item.state = state;
        if finished {
            item.total = Some(item.total.unwrap_or(item.received).max(item.received));
            item.received = item.total.unwrap_or(item.received);
        }
        self.revision.changed();
        Some(ProgressUpdate {
            item: item.key,
            stored: stored(item, moved),
            finished,
        })
    }

    pub fn unlink(&mut self, web: &WebId) {
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.web.as_ref() == Some(web))
        {
            item.web = None;
            self.revision.changed();
        }
    }

    pub fn merge(&mut self, stored: Vec<Download>) {
        let known: HashSet<DownloadKey> = self.items.iter().map(|item| item.key).collect();
        let fresh: Vec<DownloadItem> = stored
            .into_iter()
            .map(DownloadItem::from_stored)
            .filter(|item| !known.contains(&item.key))
            .collect();
        if fresh.is_empty() {
            return;
        }
        self.items.extend(fresh);
        self.items
            .sort_by_key(|item| std::cmp::Reverse(item.started));
        self.revision.changed();
    }

    pub fn remove(&mut self, key: DownloadKey) -> Option<DownloadItem> {
        let at = self.items.iter().position(|item| item.key == key)?;
        self.revision.changed();
        Some(self.items.remove(at))
    }

    pub fn forget(&mut self, profile: ProfileKey) {
        let before = self.items.len();
        self.items.retain(|item| item.profile != profile);
        if self.items.len() != before {
            self.revision.changed();
        }
    }

    pub fn clear_ended(&mut self, profile: ProfileKey) -> Vec<DownloadItem> {
        let (ended, remaining): (Vec<DownloadItem>, Vec<DownloadItem>) =
            std::mem::take(&mut self.items)
                .into_iter()
                .partition(|item| item.profile == profile && item.state.ended());
        self.items = remaining;
        if !ended.is_empty() {
            self.revision.changed();
        }
        ended
    }
}

fn stored(item: &DownloadItem, moved: bool) -> Option<StoredDownload> {
    let DownloadKey::StoredDownload(id) = item.key else {
        return None;
    };
    Some(StoredDownload {
        id,
        progress: browser_store::DownloadProgress {
            received: item.received,
            total: item.total,
            state: item.state.stored(),
        },
        path: item.path.clone().filter(|_| moved),
    })
}
