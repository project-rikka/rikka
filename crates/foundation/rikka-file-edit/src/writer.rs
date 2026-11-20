use std::io::Write;
use std::path::Path;

use rikka_fs::Replace;

use crate::error::{Error, Result};

#[derive(Debug)]
pub(crate) enum Precondition<'a> {
    Any,
    Exactly(&'a [u8]),
}

pub(crate) fn write(path: &Path, bytes: &[u8], precondition: &Precondition<'_>) -> Result<()> {
    let mut staged = Replace::new(path)
        .stage()
        .map_err(|error| io_error(path, &error))?;
    staged
        .write_all(bytes)
        .map_err(|error| io_error(path, &error))?;

    if let Precondition::Exactly(expected) = precondition {
        let now = std::fs::read(staged.target()).map_err(|error| io_error(path, &error))?;
        if now != *expected {
            return Err(Error::ConcurrentModification(path.display().to_string()));
        }
    }

    staged.commit().map_err(|error| io_error(path, &error))
}

fn io_error(path: &Path, error: &std::io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}
