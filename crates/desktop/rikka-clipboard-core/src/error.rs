#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("clipboard unavailable: {0}")]
    Unavailable(&'static str),

    #[error("clipboard is empty")]
    Empty,

    #[error("clipboard has no \"{0}\" offer")]
    NoSuchType(String),

    #[error("clipboard text is not UTF-8")]
    NotUtf8,

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
