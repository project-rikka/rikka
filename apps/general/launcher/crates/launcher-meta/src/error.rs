#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid metadata: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid Maven coordinate \"{0}\"")]
    Coordinate(String),
    #[error("unsafe relative path \"{0}\"")]
    Path(String),
    #[error("invalid SHA-1 digest \"{0}\"")]
    Digest(String),
    #[error("version {version} has no {field}")]
    Missing {
        version: String,
        field: &'static str,
    },
    #[error("version {child} inherits from {expected}, not {found}")]
    Parent {
        child: String,
        expected: String,
        found: String,
    },
    #[error("argument \"{argument}\" names the unknown placeholder {placeholder}")]
    Placeholder {
        argument: String,
        placeholder: String,
    },
    #[error("library {0} has no download")]
    Undownloadable(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
