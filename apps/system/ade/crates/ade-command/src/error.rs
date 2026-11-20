use ade_scope::FrontMatterError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("cannot read file: {0}")]
    Read(#[source] std::io::Error),

    #[error("\"{0}\" is not a valid command name")]
    InvalidName(String),

    #[error(transparent)]
    FrontMatter(#[from] FrontMatterError),

    #[error("empty prompt")]
    Empty,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
