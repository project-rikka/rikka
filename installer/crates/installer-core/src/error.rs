use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("no disk chosen")]
    NoDisk,

    #[error("{disk} is {size}; this layout needs at least {needed}")]
    DiskTooSmall {
        disk: String,
        size: String,
        needed: String,
    },

    #[error("failed to hash password: {0}")]
    Hash(String),

    #[error("failed to read the Rikka catalog: {0}")]
    Catalog(String),

    #[error("failed to render configuration: {0}")]
    Nix(#[from] rikka_nix_edit::Error),
}
