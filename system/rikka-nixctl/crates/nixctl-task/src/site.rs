use std::path::PathBuf;

use rikka_nix::{DIRECTORY, FLAKE};

const PROGRAMS: &str = "/run/current-system/sw/bin";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub modules: PathBuf,
    pub flake: PathBuf,
    pub programs: PathBuf,
}

impl Site {
    #[must_use]
    pub fn system() -> Self {
        Self {
            modules: PathBuf::from(DIRECTORY),
            flake: PathBuf::from(FLAKE),
            programs: PathBuf::from(PROGRAMS),
        }
    }
}
