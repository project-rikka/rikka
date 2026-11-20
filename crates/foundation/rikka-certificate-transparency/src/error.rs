use std::io;
use std::path::PathBuf;

use jiff::Timestamp;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("log list signature does not verify")]
    Signature,

    #[error("malformed log list: {0}")]
    MalformedList(String),

    #[error("unsupported log list key")]
    UnsupportedKey,

    #[error("invalid log list URL \"{url}\": {reason}")]
    Url { url: String, reason: String },

    #[error("log list expired at {0}")]
    Expired(Timestamp),

    #[error("no log list has been fetched yet")]
    NotFetched,

    #[error("failed to fetch log list: {0}")]
    Fetch(#[from] rikka_net::Error),

    #[error("failed to access log list cache {}: {source}", path.display())]
    Cache {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("malformed log list cache {}: {reason}", path.display())]
    MalformedCache { path: PathBuf, reason: String },

    #[error("malformed certificate: {0}")]
    MalformedCertificate(String),

    #[error("malformed SCT list")]
    MalformedSctList,
}
