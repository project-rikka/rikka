use std::collections::HashMap;

use rikka_ui::Revision;
use rikka_web::{DownloadId, DownloadItem, DownloadState};

use crate::ProfileKey;

const USER_CANCELED: &str = "USER_CANCELED";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DownloadPhase {
    Running,
    Paused,
    Finished,
    Failed,
    Cancelled,
}

impl DownloadPhase {
    pub fn of(item: &DownloadItem) -> DownloadPhase {
        match item.state {
            DownloadState::InProgress if item.paused => DownloadPhase::Paused,
            DownloadState::InProgress => DownloadPhase::Running,
            DownloadState::Complete => DownloadPhase::Finished,
            DownloadState::Interrupted if item.error.as_deref() == Some(USER_CANCELED) => {
                DownloadPhase::Cancelled
            }
            DownloadState::Interrupted => DownloadPhase::Failed,
        }
    }

    pub fn ended(self) -> bool {
        matches!(
            self,
            DownloadPhase::Finished | DownloadPhase::Failed | DownloadPhase::Cancelled
        )
    }

    fn noticed(self) -> bool {
        matches!(self, DownloadPhase::Finished | DownloadPhase::Failed)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DownloadActivity {
    pub running: usize,
    pub fraction: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EndedDownload {
    pub profile: ProfileKey,
    pub phase: DownloadPhase,
    pub item: DownloadItem,
}

#[derive(Debug, Default)]
pub struct Downloads {
    by_profile: HashMap<ProfileKey, Vec<DownloadItem>>,
    revision: Revision,
}

impl Downloads {
    pub fn items(&self, profile: ProfileKey) -> &[DownloadItem] {
        self.revision.read();
        self.by_profile.get(&profile).map_or(&[], Vec::as_slice)
    }

    pub fn item(&self, profile: ProfileKey, id: DownloadId) -> Option<&DownloadItem> {
        self.items(profile).iter().find(|item| item.id == id)
    }

    pub fn activity(&self, profile: ProfileKey) -> DownloadActivity {
        let running: Vec<&DownloadItem> = self
            .items(profile)
            .iter()
            .filter(|item| !DownloadPhase::of(item).ended())
            .collect();
        let known = running.iter().all(|item| item.total_bytes > 0.0);
        let (received, total) = running.iter().fold((0.0, 0.0), |(received, total), item| {
            (received + item.bytes_received, total + item.total_bytes)
        });
        DownloadActivity {
            running: running.len(),
            fraction: (known && total > 0.0).then(|| (received / total).clamp(0.0, 1.0) as f32),
        }
    }

    pub fn update(
        &mut self,
        profile: ProfileKey,
        snapshot: Vec<DownloadItem>,
    ) -> Vec<EndedDownload> {
        let ended = match self.by_profile.get(&profile) {
            Some(previous) => newly_ended(profile, previous, &snapshot),
            None => Vec::new(),
        };
        if self.by_profile.get(&profile) != Some(&snapshot) {
            self.by_profile.insert(profile, snapshot);
            self.revision.changed();
        }
        ended
    }

    pub fn forget(&mut self, profile: ProfileKey) {
        if self.by_profile.remove(&profile).is_some() {
            self.revision.changed();
        }
    }
}

fn newly_ended(
    profile: ProfileKey,
    previous: &[DownloadItem],
    snapshot: &[DownloadItem],
) -> Vec<EndedDownload> {
    let newest = previous.iter().map(|item| item.id).max();
    snapshot
        .iter()
        .filter_map(|item| {
            let phase = DownloadPhase::of(item);
            if !phase.noticed() {
                return None;
            }
            let fresh = match previous.iter().find(|known| known.id == item.id) {
                Some(known) => !DownloadPhase::of(known).ended(),
                None => newest.is_none_or(|newest| item.id > newest),
            };
            fresh.then(|| EndedDownload {
                profile,
                phase,
                item: item.clone(),
            })
        })
        .collect()
}

pub fn download_name(item: &DownloadItem) -> String {
    if let Some(name) = item.filename.file_name() {
        return name.to_string_lossy().into_owned();
    }
    let address = match item.final_url.is_empty() {
        true => &item.url,
        false => &item.final_url,
    };
    url::Url::parse(address)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_owned))
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| address.clone())
}
