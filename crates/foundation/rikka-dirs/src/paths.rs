use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const APP: &str = "rikka";

const THEMES: &str = "themes";

#[derive(Clone, Debug)]
pub struct Paths {
    config: PathBuf,
    state: PathBuf,
    data: PathBuf,
    cache: PathBuf,
    runtime: Option<PathBuf>,
    system_config: Vec<PathBuf>,
    system_data: Vec<PathBuf>,
}

impl Paths {
    pub fn resolve() -> Result<Self> {
        let under = |home: Option<PathBuf>| home.map(|home| home.join(APP)).ok_or(Error::NoHome);
        let config = under(crate::base::config_home())?;
        let data = under(crate::base::data_home())?;
        Ok(Self {
            system_config: system(crate::base::config_dirs(), &config),
            system_data: system(crate::base::data_dirs(), &data),
            config,
            data,
            state: under(crate::base::state_home())?,
            cache: crate::base::app_cache(APP).ok_or(Error::NoHome)?,
            runtime: crate::base::runtime_dir().map(|runtime| runtime.join(APP)),
        })
    }

    #[must_use]
    pub fn themes(&self) -> Vec<PathBuf> {
        self.data_search(THEMES)
    }

    #[must_use]
    pub fn themes_dir(&self) -> PathBuf {
        self.data.join(THEMES)
    }

    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config
    }

    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    #[must_use]
    pub fn system_config_dirs(&self) -> &[PathBuf] {
        &self.system_config
    }

    #[must_use]
    pub fn state_dir(&self) -> &Path {
        &self.state
    }

    #[must_use]
    pub fn runtime_dir(&self) -> Option<&Path> {
        self.runtime.as_deref()
    }

    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache
    }

    fn data_search(&self, kind: &str) -> Vec<PathBuf> {
        std::iter::once(&self.data)
            .chain(&self.system_data)
            .map(|dir| dir.join(kind))
            .collect()
    }
}

pub fn ensure(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|source| Error::CreateDir {
        path: dir.to_path_buf(),
        source,
    })
}

fn system(dirs: Vec<PathBuf>, own: &Path) -> Vec<PathBuf> {
    dirs.into_iter()
        .filter(|dir| dir != own.parent().unwrap_or(own))
        .map(|dir| dir.join(APP))
        .collect()
}
