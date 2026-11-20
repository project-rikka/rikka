use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{}: {source}", .path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to reach the keyring: {0}")]
    Keyring(#[from] rikka_secrets::Error),

    #[error("no free account folder in {}", .0.display())]
    NoFreeFolder(PathBuf),
}

impl Error {
    pub(crate) fn io(path: &std::path::Path) -> impl FnOnce(std::io::Error) -> Error {
        let path = path.to_owned();
        move |source| Error::Io { path, source }
    }

    pub(crate) fn json(path: &std::path::Path) -> impl FnOnce(serde_json::Error) -> Error {
        let path = path.to_owned();
        move |source| Error::Json { path, source }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
