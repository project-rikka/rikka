use std::fmt;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("malformed page message: {0} is missing or has the wrong type")]
    Malformed(&'static str),

    #[error("unknown page message kind \"{0}\"")]
    UnknownKind(String),

    #[error("page message exceeds the {0} limit")]
    TooLarge(Limit),

    #[error("duplicate id {0} in unit")]
    DuplicateId(u32),

    #[error("{service} cannot translate into {language}")]
    Unsupported {
        service: &'static str,
        language: &'static str,
    },

    #[error("cannot create translation client")]
    Client(#[source] rikka_net::Error),

    #[error("cannot reach translation service")]
    Network(#[source] rikka_net::Error),

    #[error("translation request timed out")]
    Timeout,

    #[error("translation service rejected the request")]
    Refused,

    #[error("translation service rate limit exceeded")]
    RateLimited,

    #[error("translation service returned status {0}")]
    Status(u16),

    #[error("translation response exceeds {0} bytes")]
    ResponseTooLarge(usize),

    #[error("malformed translation response")]
    MalformedResponse,

    #[error("cannot obtain access key for {0}")]
    KeyUnavailable(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limit {
    Units,
    UnitText,
    Depth,
    Id,
    Sample,
    Language,
}

impl fmt::Display for Limit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Limit::Units => "units per message",
            Limit::UnitText => "characters per unit",
            Limit::Depth => "nesting depth",
            Limit::Id => "node ids",
            Limit::Sample => "sample length",
            Limit::Language => "language tag length",
        })
    }
}

impl From<rikka_net::Error> for Error {
    fn from(error: rikka_net::Error) -> Error {
        match error {
            rikka_net::Error::Timeout => Error::Timeout,
            rikka_net::Error::Status(status) => Error::Status(status.status().as_u16()),
            rikka_net::Error::TooLarge { limit } => Error::ResponseTooLarge(limit),
            other => Error::Network(other),
        }
    }
}
