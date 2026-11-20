use rusqlite::ErrorCode;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("failed to open sync state: {0}")]
    Open(String),
    #[error("unknown sync state schema version {found}")]
    UnknownSchema { found: i64 },
    #[error("sync state is corrupt")]
    Corrupt,
    #[error("disk holding sync state is full")]
    DiskFull,
    #[error("sync state is locked by another process")]
    Busy,
    #[error("sync state is read-only")]
    ReadOnly,
    #[error("sync state database error: {0}")]
    Database(String),
    #[error("invalid journaled operation: {0}")]
    Journal(String),
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Error {
        match error.sqlite_error_code() {
            Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => Error::Corrupt,
            Some(ErrorCode::DiskFull) => Error::DiskFull,
            Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => Error::Busy,
            Some(ErrorCode::ReadOnly) => Error::ReadOnly,
            Some(ErrorCode::CannotOpen | ErrorCode::PermissionDenied) => {
                Error::Open(error.to_string())
            }
            _ => Error::Database(error.to_string()),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Error {
        Error::Journal(error.to_string())
    }
}
