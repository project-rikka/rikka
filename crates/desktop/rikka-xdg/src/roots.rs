use std::path::{Path, PathBuf};

use crate::entry::desktops;

#[derive(Clone, Debug)]
pub(crate) struct Roots {
    pub(crate) config_home: PathBuf,
    pub(crate) config_dirs: Vec<PathBuf>,
    pub(crate) data_home: PathBuf,
    pub(crate) data_dirs: Vec<PathBuf>,
    pub(crate) desktops: Vec<String>,
}

impl Roots {
    pub(crate) fn session() -> Option<Roots> {
        let config_home = rikka_dirs::config_home()?;
        let data_home = rikka_dirs::data_home()?;
        let mut roots = Roots::new(config_home, data_home);
        roots.config_dirs = rikka_dirs::config_dirs()
            .into_iter()
            .filter(|dir| *dir != roots.config_home)
            .collect();
        roots.data_dirs = rikka_dirs::data_dirs()
            .into_iter()
            .filter(|dir| *dir != roots.data_home)
            .collect();
        roots.set_desktops(desktops());
        Some(roots)
    }

    pub(crate) fn new(config_home: PathBuf, data_home: PathBuf) -> Roots {
        Roots {
            config_home,
            config_dirs: Vec::new(),
            data_home,
            data_dirs: Vec::new(),
            desktops: Vec::new(),
        }
    }

    pub(crate) fn set_desktops(&mut self, names: impl IntoIterator<Item = impl AsRef<str>>) {
        self.desktops = names
            .into_iter()
            .map(|name| name.as_ref().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
            .collect();
    }

    pub(crate) fn desktop_lists(&self, dir: &Path, list: &str) -> Vec<PathBuf> {
        self.desktops
            .iter()
            .map(|desktop| dir.join(format!("{desktop}-{list}")))
            .collect()
    }
}
