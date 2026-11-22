use std::path::PathBuf;

use rikka_nix::Location;

const PROGRAMS: &str = "/run/current-system/sw/bin";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    pub modules: PathBuf,
    pub flake: PathBuf,
    pub programs: PathBuf,
}

impl Site {
    pub fn system() -> rikka_nix::Result<Self> {
        let location = Location::system()?;
        Ok(Self {
            modules: location.modules().to_owned(),
            flake: location.flake().to_owned(),
            programs: PathBuf::from(PROGRAMS),
        })
    }
}
