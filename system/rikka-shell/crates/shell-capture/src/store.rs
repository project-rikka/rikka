use std::path::{Path, PathBuf};

use rikka_dirs::{UserDir, UserDirs};

use crate::error::{Error, Result};

const MAX_ALIKE: u32 = 100;

pub(crate) fn folder(named: Option<&Path>, under: UserDir, own: &str) -> Result<PathBuf> {
    if let Some(named) = named {
        return Ok(named.to_path_buf());
    }
    Ok(UserDirs::load()
        .path(under)
        .or_else(|| Some(rikka_dirs::home()?.to_path_buf()))
        .ok_or(Error::Nowhere)?
        .join(own))
}

pub(crate) fn ensure(folder: &Path) -> Result<()> {
    std::fs::create_dir_all(folder).map_err(Error::NoFolder)
}

#[must_use]
pub(crate) fn file(folder: &Path, what: &str, extension: &str) -> PathBuf {
    let when = jiff::Zoned::now().strftime("%Y-%m-%d %H-%M-%S");
    free(folder, &format!("{what} {when}"), extension)
}

#[must_use]
fn free(folder: &Path, stem: &str, extension: &str) -> PathBuf {
    let first = folder.join(format!("{stem}.{extension}"));
    if !first.exists() {
        return first;
    }
    (2..MAX_ALIKE)
        .map(|count| folder.join(format!("{stem} ({count}).{extension}")))
        .find(|path| !path.exists())
        .unwrap_or(first)
}
