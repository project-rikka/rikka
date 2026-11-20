#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot build cloth from model: {0}")]
    Malformed(#[source] rikka_model_core::Error),

    #[error(transparent)]
    File(#[from] rikka_file_format::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
