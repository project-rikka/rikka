use std::io::ErrorKind;
use std::path::Path;

use rikka_fs::Replace;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

pub fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    Replace::new(path)
        .private()
        .write(bytes)
        .map_err(Error::io(path))
}

pub fn write_private_json(path: &Path, value: &impl Serialize) -> Result<()> {
    write_private(path, &serde_json::to_vec(value).map_err(Error::json(path))?)
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::io(path)(error)),
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(Error::json(path))
}
