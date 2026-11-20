use ade_scope::FrontMatterError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("cannot read file: {0}")]
    Read(#[source] std::io::Error),

    #[error(transparent)]
    FrontMatter(#[from] FrontMatterError),

    #[error("missing description")]
    MissingDescription,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
