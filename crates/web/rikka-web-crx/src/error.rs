#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read package: {0}")]
    Archive(#[from] zip::result::ZipError),
    #[error("failed to write package: {0}")]
    Io(#[from] std::io::Error),
    #[error("package has no manifest.json")]
    MissingManifest,
    #[error("manifest.json is invalid: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("manifest.json is not an object")]
    ManifestShape,
    #[error("signed header contains a zip end-of-directory marker")]
    HeaderMarker,
    #[error("invalid version {0:?}")]
    Version(String),
    #[error("package path {0:?} leaves the package")]
    UnsafePath(String),
}

pub type Result<T> = std::result::Result<T, Error>;
