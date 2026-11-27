use std::env;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

const CHROME_OVERRIDE: &str = "RIKKA_CHROME_PATH";

const CHROME_INSTALLED: &str = "/etc/rikka/web/chrome";

const HOST_OVERRIDE: &str = "RIKKA_WEB_HOST_PATH";

const HOST_INSTALLED: &str = "/etc/rikka/web/host";

const HOST_PROGRAM: &str = "rikka-web-host";

pub(crate) fn chrome() -> Result<PathBuf> {
    let path =
        env::var_os(CHROME_OVERRIDE).map_or_else(|| PathBuf::from(CHROME_INSTALLED), PathBuf::from);
    absolute(path).ok_or(Error::ChromeNotInstalled)
}

pub(crate) fn host() -> Result<PathBuf> {
    if let Some(path) = env::var_os(HOST_OVERRIDE) {
        return absolute(PathBuf::from(path)).ok_or(Error::HostNotInstalled);
    }
    let program = env::current_exe().map_err(|_| Error::HostNotInstalled)?;
    let folder = program.parent().ok_or(Error::HostNotInstalled)?;
    candidates(folder)
        .into_iter()
        .find(|path| path.is_file())
        .ok_or(Error::HostNotInstalled)
}

fn absolute(path: PathBuf) -> Option<PathBuf> {
    path.is_file()
        .then(|| std::fs::canonicalize(&path).ok())
        .flatten()
}

fn candidates(folder: &Path) -> [PathBuf; 3] {
    [
        folder.join(HOST_PROGRAM),
        folder
            .parent()
            .map_or_else(PathBuf::new, |prefix| prefix.join("bin").join(HOST_PROGRAM)),
        PathBuf::from(HOST_INSTALLED),
    ]
}
