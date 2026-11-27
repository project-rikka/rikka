use std::path::PathBuf;

use rikka_dirs::{UserDir, UserDirs};

use crate::error::{Error, Result};

const FOLDER: &str = "Animate";

pub(crate) fn root() -> Result<PathBuf> {
    let documents = UserDirs::load()
        .path(UserDir::Documents)
        .ok_or(Error::NoDocuments)?;
    Ok(documents.join(FOLDER))
}
