#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("screen cast portal unavailable: {0}")]
    Unavailable(String),

    #[error("no source selected")]
    Cancelled,

    #[error("screen cast request denied")]
    Refused,

    #[error("invalid screen cast portal response: {0}")]
    Malformed(String),

    #[error("PipeWire failed: {0}")]
    PipeWire(String),

    #[error("cannot signal captured frames: {0}")]
    Signal(std::io::ErrorKind),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn unavailable(error: impl std::fmt::Display) -> Error {
        Error::Unavailable(error.to_string())
    }

    pub(crate) fn pipewire(error: impl std::fmt::Display) -> Error {
        Error::PipeWire(error.to_string())
    }
}

impl From<rikka_worker::Error<Error>> for Error {
    fn from(error: rikka_worker::Error<Error>) -> Error {
        match error {
            rikka_worker::Error::Startup(error) => error,
            other => Error::pipewire(other),
        }
    }
}
