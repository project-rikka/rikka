#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("file chooser unavailable: {0}")]
    Unavailable(String),

    #[error("file chooser closed without a response")]
    NoResponse,

    #[error("invalid file chooser response: {0}")]
    Malformed(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
