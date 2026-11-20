use std::error::Error as StdError;
use std::path::PathBuf;

pub type Cause = Box<dyn StdError + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read theme file `{path}`")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read themes directory `{path}`")]
    ReadDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid theme definition")]
    Parse(#[source] Cause),

    #[error("invalid theme slug `{0}`: expected lowercase letters, digits, `-` or `_`")]
    InvalidSlug(String),

    #[error("invalid theme opacity `{0}`: expected 0.0 to 1.0")]
    InvalidOpacity(f32),

    #[error(transparent)]
    Dirs(#[from] rikka_dirs::Error),
}

impl From<toml::de::Error> for Error {
    fn from(source: toml::de::Error) -> Self {
        Self::Parse(Box::new(source))
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
