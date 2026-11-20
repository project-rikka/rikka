pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("search engine id is empty")]
    MissingId,

    #[error("search engine name is empty")]
    MissingName,

    #[error("\"{0}\" cannot be a search engine keyword")]
    InvalidKeyword(String),

    #[error("search engine URL is empty")]
    MissingTemplate,

    #[error("\"{0}\" is not a valid http or https search URL template")]
    InvalidTemplate(String),

    #[error("\"{0}\" has no %s or {{searchTerms}} placeholder")]
    MissingPlaceholder(String),

    #[error("duplicate search engine id \"{0}\"")]
    DuplicateId(String),

    #[error("duplicate search engine keyword \"{0}\"")]
    DuplicateKeyword(String),

    #[error("no search engine with id \"{0}\"")]
    UnknownEngine(String),

    #[error("cannot remove built-in search engine \"{0}\"")]
    BuiltinEngine(String),

    #[error("\"{0}\" is not a valid URL")]
    InvalidUrl(String),

    #[error("suggestion request timed out")]
    Timeout,

    #[error("cannot reach suggestion service")]
    Network(#[source] rikka_net::Error),

    #[error("suggestion service returned status {0}")]
    Status(u16),

    #[error("suggestion response exceeds {0} bytes")]
    TooLarge(usize),

    #[error("malformed suggestion response")]
    Malformed,
}

impl From<rikka_net::Error> for Error {
    fn from(error: rikka_net::Error) -> Error {
        match error {
            rikka_net::Error::Timeout => Error::Timeout,
            rikka_net::Error::Status(status) => Error::Status(status.status().as_u16()),
            rikka_net::Error::TooLarge { limit } => Error::TooLarge(limit),
            other => Error::Network(other),
        }
    }
}
