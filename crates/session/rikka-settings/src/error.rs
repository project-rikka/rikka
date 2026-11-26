use std::error::Error as StdError;
use std::path::PathBuf;

pub type Cause = Box<dyn StdError + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(
        "`{0}` is not a valid settings domain: expected lowercase reverse-DNS, such as `com.example.notes`"
    )]
    InvalidDomain(String),

    #[error("invalid schema for `{domain}`: {reason}")]
    BadDeclaration { domain: String, reason: String },

    #[error("schema for `{domain}` declares `{key}` twice")]
    DuplicateKey { domain: String, key: String },

    #[error("`{domain}` declares no setting called `{key}`")]
    UnknownKey { domain: String, key: String },

    #[error("invalid value for `{key}`: {reason}")]
    Refused { key: String, reason: String },

    #[error("`{domain}/{key}` is locked by the system configuration")]
    Locked { domain: String, key: String },

    #[error("the catalog describes `{0}` twice")]
    DuplicateDomain(String),

    #[error("failed to read the settings catalog: {0}")]
    Catalog(String),

    #[error("value conversion failed")]
    Convert(#[source] Cause),

    #[error("{0}")]
    Unrepresentable(String),

    #[error("failed to read settings file `{path}`")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to write settings file `{path}`")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to encode settings for `{domain}`")]
    Encode {
        domain: String,
        #[source]
        source: Cause,
    },

    #[error(transparent)]
    Dirs(#[from] rikka_dirs::Error),

    #[error("settings service rejected the change: {message}")]
    Session { code: u16, message: String },
}

impl Error {
    pub(crate) fn refused(key: &str, reason: String) -> Self {
        Self::Refused {
            key: key.to_owned(),
            reason,
        }
    }

    #[must_use]
    pub fn from_seam(error: rikka_seam::SeamError) -> Self {
        let code = error.code();
        match code / 100 {
            3 => Self::Refused {
                key: "the setting".to_owned(),
                reason: error.to_string(),
            },
            _ => Self::Session {
                code,
                message: error.to_string(),
            },
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
