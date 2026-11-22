use std::io;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};
use crate::module::Module;

const CONFIGURATION: &str = "/etc/rikka/configuration.json";

const STANDARD_FLAKE: &str = "/etc/nixos";

const STANDARD_MODULES: &str = "rikka";

const FLAKE_FILE: &str = "flake.nix";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declared {
    flake: PathBuf,
    modules: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    flake: PathBuf,
    modules: PathBuf,
}

impl Location {
    #[must_use]
    pub fn standard() -> Self {
        let flake = PathBuf::from(STANDARD_FLAKE);
        Self {
            modules: flake.join(STANDARD_MODULES),
            flake,
        }
    }

    pub fn system() -> Result<Self> {
        Self::read(CONFIGURATION)
    }

    pub fn read(configuration: impl AsRef<Path>) -> Result<Self> {
        let path = configuration.as_ref();
        let json = match std::fs::read_to_string(path) {
            Ok(json) => json,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(Error::Unmanaged(path.to_owned()));
            }
            Err(source) => {
                return Err(Error::Unreadable {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        let declared: Declared =
            serde_json::from_str(&json).map_err(|source| Error::Malformed {
                path: path.to_owned(),
                source,
            })?;
        let location = Self::declared(declared)?;
        location.adopted()?;
        Ok(location)
    }

    #[must_use]
    pub fn flake(&self) -> &Path {
        &self.flake
    }

    #[must_use]
    pub fn flake_file(&self) -> PathBuf {
        self.flake.join(FLAKE_FILE)
    }

    #[must_use]
    pub fn modules(&self) -> &Path {
        &self.modules
    }

    #[must_use]
    pub fn module(&self, module: Module) -> PathBuf {
        self.modules.join(module.file_name())
    }

    fn declared(Declared { flake, modules }: Declared) -> Result<Self> {
        for path in [&flake, &modules] {
            if !is_normal_absolute(path) {
                return Err(Error::NotAbsolute(path.clone()));
            }
        }
        if modules == flake || !modules.starts_with(&flake) {
            return Err(Error::Outside { modules, flake });
        }
        Ok(Self { flake, modules })
    }

    fn adopted(&self) -> Result<()> {
        if !self.flake_file().is_file() {
            return Err(Error::NoFlake(self.flake.clone()));
        }
        if !self.module(Module::Root).is_file() {
            return Err(Error::NotAdopted(self.modules.clone()));
        }
        Ok(())
    }
}

fn is_normal_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}
