use std::io;
use std::path::PathBuf;

use crate::error::{Error, Result};

const FOLDER: &str = "Library";

pub(crate) fn root() -> Result<PathBuf> {
    let program = std::env::current_exe().map_err(Error::NoProgramFolder)?;
    let folder = program
        .parent()
        .ok_or_else(|| Error::NoProgramFolder(io::Error::from(io::ErrorKind::NotFound)))?;
    Ok(folder.join(FOLDER))
}
