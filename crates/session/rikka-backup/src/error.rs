pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Age(#[from] rikka_age::Error),
    #[error("backup contents: {0}")]
    Format(#[from] rikka_file_format::Error),
    #[error("backup configuration: {0}")]
    Nix(#[from] rikka_nix_edit::Error),
    #[error("the backup is damaged: {0}")]
    Invalid(String),
}
