use std::error::Error as StdError;
use std::path::PathBuf;

use crate::locale::Locale;
use crate::parser::SyntaxError;

pub type Cause = Box<dyn StdError + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read locale file `{path}`")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("`{source_name}` is not valid TOML")]
    Parse {
        source_name: String,
        #[source]
        source: Cause,
    },

    #[error("invalid locale tag `{0}`: expected something like `ja` or `pt-BR`")]
    InvalidLocale(String),

    #[error(
        "`{source_name}` does not name a locale: name the file after it (`ja.toml`) \
         or declare `[i18n] locale = \"…\"`"
    )]
    UnknownLocale { source_name: String },

    #[error("`{source_name}`: `{key}` must be a string or a table of strings, found {found}")]
    UnexpectedValue {
        source_name: String,
        key: String,
        found: &'static str,
    },

    #[error("`{source_name}`: unknown metadata key `{key}`")]
    UnknownMetadataKey { source_name: String, key: String },

    #[error("`{source_name}`: message `{key}` is malformed")]
    Message {
        source_name: String,
        key: String,
        #[source]
        source: SyntaxError,
    },

    #[error("`{source_name}` declares locale `{locale}`: name the file `{locale}.toml`")]
    MisnamedLocale { source_name: String, locale: Locale },

    #[error("missing reference catalog `en.toml`")]
    NoReference,

    #[error("`{source_name}` has no messages")]
    EmptyReference { source_name: String },

    #[error(
        "`{source_name}` disagrees with `{reference}`: missing [{}], unexpected [{}]",
        .missing.join(", "),
        .unexpected.join(", ")
    )]
    Disagrees {
        source_name: String,
        reference: String,
        missing: Vec<String>,
        unexpected: Vec<String>,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
