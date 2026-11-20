use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("empty name")]
    Empty,

    #[error("invalid name: `{0}`")]
    BadName(String),

    #[error("{} already exists", .0.display())]
    Exists(PathBuf),

    #[error("{} is outside the project", .0.display())]
    Outside(PathBuf),

    #[error(transparent)]
    Operation(#[from] rikka_file_operations::Error<PathBuf>),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
