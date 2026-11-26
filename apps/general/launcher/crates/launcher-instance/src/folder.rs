use std::fs::{self, DirEntry};
use std::io::ErrorKind;
use std::path::Path;

use crate::error::{Error, Result};

pub(crate) fn entries(folder: &Path) -> Result<Vec<DirEntry>> {
    let listed = match fs::read_dir(folder) {
        Ok(listed) => listed,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(Error::Io {
                path: folder.to_owned(),
                source,
            });
        }
    };
    listed
        .map(|entry| entry.map_err(Error::io(folder)))
        .collect()
}
