use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("documents folder not found")]
    NoDocuments,

    #[error("program folder not found: {0}")]
    NoProgramFolder(#[source] std::io::Error),

    #[error("failed to find cache folder: {0}")]
    NoCache(#[from] rikka_dirs::Error),

    #[error("failed to access `{}`: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to watch library: {0}")]
    Watch(#[from] rikka_watch::Error),

    #[error("failed to start library worker: {0}")]
    Worker(#[source] std::io::Error),

    #[error("library worker exited during startup")]
    WorkerExited,

    #[error("failed to read embedded picture: {0}")]
    Embedded(&'static str),

    #[error("failed to convert picture: {0}")]
    Image(#[from] rikka_image::Error),

    #[error("failed to read sound tags: {0}")]
    Tags(#[from] rikka_audio_tags::Error),

    #[error("`{}` has no usable modification time", .0.display())]
    Undated(PathBuf),
}

impl From<rikka_worker::Error<Error>> for Error {
    fn from(error: rikka_worker::Error<Error>) -> Error {
        match error {
            rikka_worker::Error::Spawn(source) => Error::Worker(source),
            rikka_worker::Error::Startup(error) => error,
            rikka_worker::Error::Exited => Error::WorkerExited,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) trait AtPath<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T>;
}

impl<T> AtPath<T> for std::io::Result<T> {
    fn at(self, path: impl Into<PathBuf>) -> Result<T> {
        self.map_err(|source| Error::Io {
            path: path.into(),
            source,
        })
    }
}
