use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{} does not exist", .0.display())]
    NotFound(PathBuf),

    #[error("no trash directory for {}", .0.display())]
    Unsupported(PathBuf),

    #[error("file exists: {}", .0.display())]
    Exists(PathBuf),

    #[error("{}: {source}", .path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>) -> impl FnOnce(std::io::Error) -> Error {
        let path = path.into();
        move |source| Error::Io { path, source }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
