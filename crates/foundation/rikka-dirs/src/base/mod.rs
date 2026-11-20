#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[must_use]
pub fn home() -> Option<&'static Path> {
    static HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
    HOME.get_or_init(platform::home).as_deref()
}

#[must_use]
pub fn data_home() -> Option<PathBuf> {
    named("XDG_DATA_HOME").or_else(platform::data_home)
}

#[must_use]
pub fn config_home() -> Option<PathBuf> {
    named("XDG_CONFIG_HOME").or_else(platform::config_home)
}

#[must_use]
pub fn cache_home() -> Option<PathBuf> {
    named("XDG_CACHE_HOME").or_else(platform::cache_home)
}

#[must_use]
pub fn state_home() -> Option<PathBuf> {
    named("XDG_STATE_HOME").or_else(platform::state_home)
}

#[must_use]
pub fn runtime_dir() -> Option<PathBuf> {
    platform::runtime_dir()
}

#[must_use]
pub fn data_dirs() -> Vec<PathBuf> {
    search("XDG_DATA_DIRS", data_home(), platform::system_data())
}

#[must_use]
pub fn config_dirs() -> Vec<PathBuf> {
    search("XDG_CONFIG_DIRS", config_home(), platform::system_config())
}

pub(crate) fn app_cache(app: &str) -> Option<PathBuf> {
    match named("XDG_CACHE_HOME") {
        Some(cache) => Some(cache.join(app)),
        None => platform::app_cache(app),
    }
}

fn named(variable: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

fn search(variable: &str, own: Option<PathBuf>, fallback: Vec<PathBuf>) -> Vec<PathBuf> {
    let listed = std::env::var_os(variable).filter(|value| !value.is_empty());
    ordered(own, listed.as_deref(), fallback)
}

fn ordered(own: Option<PathBuf>, listed: Option<&OsStr>, fallback: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = own.into_iter().collect();
    match listed {
        Some(value) => dirs.extend(std::env::split_paths(value).filter(|p| p.is_absolute())),
        None => dirs.extend(fallback),
    }
    let mut seen = std::collections::HashSet::new();
    dirs.retain(|dir| seen.insert(dir.clone()));
    dirs
}
