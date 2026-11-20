use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use rusqlite::{Connection, Row, params};

use crate::error::Result;
use crate::ids::{DownloadId, ProfileId};
use crate::records::count;
use crate::store::Store;
use crate::time::Timestamp;
use crate::writer::{Coalesce, WriterState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DownloadState {
    InProgress,
    Finished,
    Failed,
    Cancelled,
}

impl DownloadState {
    pub(crate) const IN_PROGRESS: i64 = 0;

    fn code(self) -> i64 {
        match self {
            Self::InProgress => Self::IN_PROGRESS,
            Self::Finished => 1,
            Self::Failed => 2,
            Self::Cancelled => 3,
        }
    }

    fn from_code(code: i64) -> Self {
        match code {
            Self::IN_PROGRESS => Self::InProgress,
            1 => Self::Finished,
            3 => Self::Cancelled,
            _ => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDownload {
    pub url: String,
    pub path: PathBuf,
    pub mime: String,
    pub total: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DownloadProgress {
    pub received: u64,
    pub total: Option<u64>,
    pub state: DownloadState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub id: DownloadId,
    pub profile: ProfileId,
    pub url: String,
    pub path: PathBuf,
    pub mime: String,
    pub total: Option<u64>,
    pub received: u64,
    pub state: DownloadState,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
}

fn stored(bytes: u64) -> i64 {
    i64::try_from(bytes).unwrap_or(i64::MAX)
}

fn loaded(bytes: i64) -> u64 {
    u64::try_from(bytes).unwrap_or(0)
}

fn download(row: &Row<'_>) -> rusqlite::Result<Download> {
    let path: Vec<u8> = row.get(3)?;
    let total: Option<i64> = row.get(5)?;
    Ok(Download {
        id: row.get(0)?,
        profile: row.get(1)?,
        url: row.get(2)?,
        path: PathBuf::from(OsStr::from_bytes(&path)),
        mime: row.get(4)?,
        total: total.map(loaded),
        received: loaded(row.get(6)?),
        state: DownloadState::from_code(row.get(7)?),
        started_at: row.get(8)?,
        finished_at: row.get(9)?,
    })
}

impl Store {
    pub fn record_download(&self, profile: ProfileId, download: NewDownload) -> DownloadId {
        let id = DownloadId::from_raw(self.sequences().download.next());
        let started_at = Timestamp::now();
        self.write(move |connection, _| {
            connection
                .prepare_cached(
                    "INSERT INTO downloads \
                     (id, profile_id, url, path, mime, total, received, state, started_at) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8)",
                )?
                .execute(params![
                    id,
                    profile,
                    download.url,
                    download.path.as_os_str().as_bytes(),
                    download.mime,
                    download.total.map(stored),
                    DownloadState::InProgress.code(),
                    started_at
                ])?;
            Ok(())
        });
        id
    }

    pub fn update_download(&self, id: DownloadId, progress: DownloadProgress) {
        let finished_at = (progress.state != DownloadState::InProgress).then(Timestamp::now);
        let job = move |connection: &Connection, _: &mut WriterState| {
            connection
                .prepare_cached(
                    "UPDATE downloads SET received = ?2, total = ?3, state = ?4, finished_at = ?5 \
                     WHERE id = ?1",
                )?
                .execute(params![
                    id,
                    stored(progress.received),
                    progress.total.map(stored),
                    progress.state.code(),
                    finished_at
                ])?;
            Ok(())
        };
        if finished_at.is_some() {
            self.write(job);
        } else {
            self.write_behind(Some(Coalesce::Download(id)), job);
        }
    }

    pub fn set_download_path(&self, id: DownloadId, path: PathBuf) {
        self.write(move |connection, _| {
            connection
                .prepare_cached("UPDATE downloads SET path = ?2 WHERE id = ?1")?
                .execute(params![id, path.as_os_str().as_bytes()])?;
            Ok(())
        });
    }

    pub fn downloads(&self, profile: ProfileId, limit: usize) -> Result<Vec<Download>> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT id, profile_id, url, path, mime, total, received, state, started_at, \
                 finished_at FROM downloads WHERE profile_id = ?1 \
                 ORDER BY started_at DESC, id DESC LIMIT ?2",
            )?;
            let rows = statement.query_map(params![profile, count(limit)], download)?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn remove_download(&self, id: DownloadId) {
        self.write(move |connection, _| {
            connection.execute("DELETE FROM downloads WHERE id = ?1", [id])?;
            Ok(())
        });
    }

    pub fn clear_finished_downloads(&self, profile: ProfileId) {
        self.write(move |connection, _| {
            connection.execute(
                "DELETE FROM downloads WHERE profile_id = ?1 AND state != ?2",
                params![profile, DownloadState::IN_PROGRESS],
            )?;
            Ok(())
        });
    }

    pub fn fail_interrupted_downloads(&self) {
        let finished_at = Timestamp::now();
        self.write(move |connection, _| {
            connection.execute(
                "UPDATE downloads SET state = ?2, finished_at = ?3 WHERE state = ?1",
                params![
                    DownloadState::IN_PROGRESS,
                    DownloadState::Failed.code(),
                    finished_at
                ],
            )?;
            Ok(())
        });
    }
}
