use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot run nix: {0}")]
    Spawn(#[source] io::Error),

    #[error("nix failed: {0}")]
    Failed(String),

    #[error("cannot read nix output: {0}")]
    Output(#[source] io::Error),

    #[error("invalid JSON from nix: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unexpected output from nix: {0}")]
    Unexpected(&'static str),

    #[error("cannot read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("cannot reach NixOS release server: {0}")]
    Network(#[source] rikka_net::Error),

    #[error("invalid release listing: {0}")]
    Listing(#[source] rikka_xml::Error),

    #[error("no release named in {0}")]
    NoRelease(String),

    #[error(transparent)]
    Catalog(nixpkgs_catalog::Error),

    #[error("cancelled")]
    Cancelled,
}

impl From<rikka_net::Error> for Error {
    fn from(error: rikka_net::Error) -> Self {
        match error.is_cancelled() {
            true => Self::Cancelled,
            false => Self::Network(error),
        }
    }
}

impl From<nixpkgs_catalog::Error> for Error {
    fn from(error: nixpkgs_catalog::Error) -> Self {
        match error {
            nixpkgs_catalog::Error::Cancelled => Self::Cancelled,
            error => Self::Catalog(error),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
