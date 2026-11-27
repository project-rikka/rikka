use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::ErrorCode;

use crate::ids::{FolderId, PinnedTabId, ProfileId, SpaceId};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("cannot create store directory {path}")]
    CreateFolder {
        path: PathBuf,
        #[source]
        source: Arc<io::Error>,
    },
    #[error("cannot open browser store: {0}")]
    Open(String),
    #[error("browser store is corrupt")]
    Corrupt,
    #[error("disk full")]
    DiskFull,
    #[error("browser store is locked by another process")]
    Busy,
    #[error("browser store is read-only")]
    ReadOnly,
    #[error("database error: {0}")]
    Database(String),
    #[error("browser store writer stopped")]
    WriterStopped,
    #[error("{count} queued writes failed, first: {first}")]
    WritesFailed { count: usize, first: Box<Error> },
    #[error("profile {0} is used by a space")]
    ProfileInUse(ProfileId),
    #[error("cannot remove the last profile")]
    LastProfile,
    #[error("{0} does not exist")]
    Missing(Item),
    #[error("folder {0} cannot move into itself")]
    FolderCycle(FolderId),
    #[error("{width}x{height} RGBA picture needs {expected} bytes, got {actual}")]
    InvalidPicture {
        width: u32,
        height: u32,
        expected: usize,
        actual: usize,
    },
    #[error("{width}x{height} picture is larger than a stored favicon")]
    OversizedPicture { width: u32, height: u32 },
    #[error("settings of profile {profile} are malformed: {reason}")]
    MalformedSettings { profile: ProfileId, reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Item {
    #[error("profile {0}")]
    Profile(ProfileId),
    #[error("space {0}")]
    Space(SpaceId),
    #[error("pinned folder {0}")]
    Folder(FolderId),
    #[error("pinned tab {0}")]
    PinnedTab(PinnedTabId),
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => Self::Corrupt,
            Some(ErrorCode::DiskFull) => Self::DiskFull,
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Self::Busy,
            Some(ErrorCode::ReadOnly) => Self::ReadOnly,
            Some(ErrorCode::CannotOpen | ErrorCode::PermissionDenied) => {
                Self::Open(error.to_string())
            }
            _ => Self::Database(error.to_string()),
        }
    }
}
