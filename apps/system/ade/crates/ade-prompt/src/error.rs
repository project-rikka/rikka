#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Document(#[from] rikka_prompt::Error),

    #[error("no document for tool \"{0}\"")]
    Unknown(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
