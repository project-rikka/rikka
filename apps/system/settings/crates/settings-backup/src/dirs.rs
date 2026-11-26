use std::path::{Path, PathBuf};

use rikka_backup::Base;
use rikka_dirs::Paths;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dirs {
    pub home: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub state: PathBuf,
}

impl Dirs {
    #[must_use]
    pub fn from_paths(home: impl Into<PathBuf>, paths: &Paths) -> Dirs {
        Dirs {
            home: home.into(),
            config: paths.config_dir().to_owned(),
            data: paths.data_dir().to_owned(),
            state: paths.state_dir().to_owned(),
        }
    }

    #[must_use]
    pub fn of(&self, base: Base) -> &Path {
        match base {
            Base::Config => &self.config,
            Base::Data => &self.data,
            Base::State => &self.state,
        }
    }

    #[must_use]
    pub fn pending(&self) -> PathBuf {
        self.state.join(rikka_backup::PENDING)
    }
}
