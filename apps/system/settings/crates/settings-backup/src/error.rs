use std::path::{Path, PathBuf};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{} has a name that is not UTF-8", .0.display())]
    Unnamed(PathBuf),
    #[error(transparent)]
    Backup(#[from] rikka_backup::Error),
    #[error("keyring: {0}")]
    Secrets(#[from] rikka_secrets::Error),
    #[error("{} does not read back as the backup just written", .0.display())]
    Unverified(PathBuf),
    #[error("git {command} failed: {reason}")]
    Git { command: String, reason: String },
}

impl Error {
    pub(crate) fn io(path: impl AsRef<Path>) -> impl FnOnce(std::io::Error) -> Error {
        let path = path.as_ref().to_owned();
        move |source| Error::Io { path, source }
    }
}
