#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use unix as platform;
#[cfg(unix)]
pub use unix::opener;
#[cfg(windows)]
use windows as platform;

use std::path::Path;

use crate::error::{Error, Result};

pub fn open(target: &str) -> Result<()> {
    if target.trim().is_empty() {
        return Err(Error::NothingToOpen);
    }
    platform::open(target)
}

pub fn reveal(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(Error::NothingToOpen);
    }
    platform::reveal(path)
}
