use music_core::Codec;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("unsupported tag format: {0}")]
    Unsupported(Codec),

    #[error("unknown music format")]
    Unknown,

    #[error("damaged music file: {0}")]
    Unreadable(String),

    #[error("cannot read music file: {0}")]
    Io(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    #[must_use]
    pub fn is_unsupported(&self) -> bool {
        matches!(self, Error::Unsupported(_) | Error::Unknown)
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Error {
        Error::Io(error.to_string())
    }
}

impl From<lofty::error::FileParseError> for Error {
    fn from(error: lofty::error::FileParseError) -> Error {
        Error::Unreadable(error.to_string())
    }
}
