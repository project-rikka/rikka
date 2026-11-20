use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a NixOS system: {} not found", .0.display())]
    NotNixos(PathBuf),
    #[error("failed to read {}: {source}", path.display())]
    Unreadable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{}: {source}", path.display())]
    Invalid {
        path: PathBuf,
        #[source]
        source: rikka_nix_edit::Error,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
