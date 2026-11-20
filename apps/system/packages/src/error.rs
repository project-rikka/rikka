#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Nixpkgs(#[from] nixpkgs_search::Error),
    #[error(transparent)]
    Configuration(#[from] rikka_nix::Error),
    #[error(transparent)]
    Folders(#[from] rikka_dirs::Error),
}
