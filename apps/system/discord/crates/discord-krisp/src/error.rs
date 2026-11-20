use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Krisp folder {folder} lacks {missing}")]
    Incomplete { folder: PathBuf, missing: String },

    #[error("failed to download Krisp: {0}")]
    Download(String),

    #[error("failed to unpack Discord package: {0}")]
    Unpack(String),

    #[error("missing C++ runtime for Krisp: {0}")]
    Runtime(String),

    #[error("failed to load Krisp: {0}")]
    Load(String),

    #[error("Krisp from {} is already loaded", .0.display())]
    AlreadyLoaded(PathBuf),

    #[error("Krisp module lacks symbol {0}")]
    Symbol(&'static str),

    #[error("Krisp SDK initialization failed (code {0})")]
    Init(i32),

    #[error("failed to open Krisp {0} session")]
    Session(&'static str),

    #[error("Krisp frame processing failed (code {0})")]
    Process(i32),

    #[error("failed to write Krisp cache: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
