#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("drive: {0}")]
    Drive(#[from] rikka_proton_drive::Error),

    #[error("this account has no Drive")]
    NoVolume,

    #[error("file is not listed yet")]
    Unlisted,

    #[error("no private runtime directory for viewing files")]
    NoRoom,

    #[error("cannot start viewer: {0}")]
    Viewer(rikka_open::Error),

    #[error("{path}: {source}")]
    File {
        path: String,
        source: std::io::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
