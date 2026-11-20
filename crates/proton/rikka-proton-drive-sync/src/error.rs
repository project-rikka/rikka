use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Drive(#[from] rikka_proton_drive::Error),

    #[error(transparent)]
    State(#[from] rikka_proton_drive_sync_state::Error),

    #[error("I/O error on {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("no sync folder configured")]
    NoFolders,

    #[error("no state folder configured")]
    NoStateFolder,

    #[error("{0} is not a directory")]
    NotAFolder(PathBuf),

    #[error("{inner} is inside synced folder {outer}")]
    NestedFolders { outer: PathBuf, inner: PathBuf },

    #[error("file name of {0} is not valid UTF-8")]
    InvalidName(PathBuf),

    #[error("no active Drive volume")]
    NoVolume,

    #[error("failed to watch {0}: {1}")]
    Watch(PathBuf, String),

    #[error("downloaded content of {0} does not match its digest")]
    Corrupted(PathBuf),

    #[error("sync stopped")]
    Stopped,
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
        let path = path.into();
        move |source| Error::Io { path, source }
    }
}

impl Error {
    #[must_use]
    pub fn api(&self) -> Option<&rikka_proton_api::Error> {
        match self {
            Error::Drive(error) => error.api(),
            _ => None,
        }
    }
}
