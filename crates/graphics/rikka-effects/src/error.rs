use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read {path}: {source}")]
    Unreadable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("not a .cube or .3dl lookup table")]
    NotALookupTable,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
