#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("portal helper pipe error: {0}")]
    Pipe(#[from] std::io::Error),
    #[error("invalid portal helper message: {0}")]
    Malformed(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
