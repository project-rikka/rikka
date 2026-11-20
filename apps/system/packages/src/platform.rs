use std::path::PathBuf;

use nixpkgs_search::Pin;
use rikka_nix::Channel;

const SHELF: &str = "packages";

pub(crate) const SYSTEM: &str = "x86_64-linux";

pub(crate) fn pin() -> Pin {
    let version = rikka_nix::running_version().unwrap_or_default();
    Pin::new(Channel::detect().name(), &version)
}

pub(crate) fn catalog_cache() -> rikka_dirs::Result<PathBuf> {
    Ok(rikka_dirs::Paths::resolve()?.cache_dir().join(SHELF))
}
