use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is damaged: {source}")]
    Config {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{path} is not valid NBT: {source}")]
    Nbt {
        path: PathBuf,
        #[source]
        source: launcher_nbt::Error,
    },
    #[error("there is no instance {0}")]
    NoInstance(String),
    #[error("\"{0}\" is not a valid file name")]
    FileName(String),
    #[error("{0} is already installed")]
    Exists(String),
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
        let path = path.into();
        move |source| Error::Io { path, source }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
