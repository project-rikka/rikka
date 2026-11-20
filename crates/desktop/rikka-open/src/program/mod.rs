#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use std::path::{Path, PathBuf};

pub use platform::is_executable;

#[must_use]
pub fn is_runnable(program: &str) -> bool {
    if program.is_empty() {
        return false;
    }
    match program.contains(platform::SEPARATORS) {
        true => is_executable(Path::new(program)),
        false => find(program).is_some(),
    }
}

#[must_use]
pub fn path_dirs() -> Vec<PathBuf> {
    let Some(path) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    std::env::split_paths(&path)
        .filter(|directory| !directory.as_os_str().is_empty())
        .collect()
}

#[must_use]
pub fn find(program: &str) -> Option<PathBuf> {
    path_dirs()
        .into_iter()
        .find_map(|directory| platform::runnable_in(&directory, program))
}
