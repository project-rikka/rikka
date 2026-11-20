use std::path::{Path, PathBuf};

#[cfg(unix)]
const SYSTEM_CONFIG_DIRS: &[&str] = &["/etc/fonts"];
#[cfg(windows)]
const SYSTEM_CONFIG_DIRS: &[&str] = &[];
#[cfg(unix)]
const DEFAULT_CONFIG_NAME: &str = "fonts.conf";

#[derive(Clone, Debug, Default)]
pub(crate) struct Environment {
    pub(crate) home: Option<PathBuf>,
    pub(crate) cwd: Option<PathBuf>,
    pub(crate) config_home: Option<PathBuf>,
    pub(crate) data_home: Option<PathBuf>,
    pub(crate) config_path: Vec<PathBuf>,
    #[cfg(unix)]
    pub(crate) config_file: Option<PathBuf>,
}

impl Environment {
    pub(crate) fn from_process() -> Self {
        let set = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
        let mut config_path: Vec<PathBuf> = set("FONTCONFIG_PATH")
            .map(|value| {
                std::env::split_paths(&value)
                    .filter(|path| !path.as_os_str().is_empty())
                    .collect()
            })
            .unwrap_or_default();
        config_path.extend(SYSTEM_CONFIG_DIRS.iter().map(PathBuf::from));
        Self {
            home: rikka_dirs::home().map(Path::to_path_buf),
            cwd: std::env::current_dir().ok(),
            config_home: rikka_dirs::config_home(),
            data_home: rikka_dirs::data_home(),
            #[cfg(unix)]
            config_file: set("FONTCONFIG_FILE").map(PathBuf::from),
            config_path,
        }
    }

    #[cfg(unix)]
    pub(crate) fn main_config(&self) -> PathBuf {
        let name = self
            .config_file
            .clone()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_NAME));
        self.find_config(&name).unwrap_or(name)
    }

    pub(crate) fn find_config(&self, name: &Path) -> Option<PathBuf> {
        if let Some(expanded) = self.expand_home(name) {
            return Some(expanded);
        }
        if name.is_absolute() {
            return Some(name.to_owned());
        }
        self.config_path
            .iter()
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.exists())
    }

    pub(crate) fn expand_home(&self, path: &Path) -> Option<PathBuf> {
        let rest = path.strip_prefix("~").ok()?;
        Some(self.home.as_ref()?.join(rest))
    }
}
