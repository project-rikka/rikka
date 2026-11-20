#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Unreachable(String),

    #[error("{0}")]
    Malformed(String),

    #[error("portal request ended without a response")]
    Silent,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<zbus::Error> for Error {
    fn from(error: zbus::Error) -> Error {
        Error::Unreachable(error.to_string())
    }
}
