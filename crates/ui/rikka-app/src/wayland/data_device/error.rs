#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("drag and drop unavailable: {0}")]
    Unavailable(&'static str),

    #[error("failed to bind data device: {0}")]
    Unreachable(#[from] rikka_wayland::Error),

    #[error("no pointer button pressed to start drag")]
    Ungrabbed,

    #[error("drag already in progress")]
    Busy,

    #[error("drop does not offer `{0}`")]
    NotOffered(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<rikka_worker::SendError> for Error {
    fn from(error: rikka_worker::SendError) -> Self {
        match error {
            rikka_worker::SendError::Disconnected => Error::Unavailable("drag worker stopped"),
            rikka_worker::SendError::Timeout => Error::Unavailable("drag worker timed out"),
            rikka_worker::SendError::Io(error) => Error::Io(error),
        }
    }
}

pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn startup(error: rikka_worker::Error<Error>) -> Error {
    match error {
        rikka_worker::Error::Spawn(error) => Error::Io(error),
        rikka_worker::Error::Startup(error) => error,
        rikka_worker::Error::Exited => Error::Unavailable("the drag thread died on startup"),
    }
}
