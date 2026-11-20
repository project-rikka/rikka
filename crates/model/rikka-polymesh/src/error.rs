use rikka_file_format::DecodeError;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid polygon mesh: {0}")]
    Invalid(String),

    #[error(transparent)]
    Decode(#[from] DecodeError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
