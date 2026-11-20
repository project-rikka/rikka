use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a NixOS system: {0} not found")]
    NotNixos(PathBuf),
    #[error("failed to read {path}: {reason}")]
    Unreadable { path: PathBuf, reason: String },
    #[error("failed to read the Rikka catalog: {0}")]
    Catalog(String),
    #[error(transparent)]
    Layout(#[from] rikka_nixos_layout::Error),
    #[error(transparent)]
    Edit(#[from] rikka_nix_edit::Error),
    #[error("authorization denied")]
    Denied,
    #[error("failed to run rikka-nixctl: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Protocol(#[from] rikka_nix_protocol::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
