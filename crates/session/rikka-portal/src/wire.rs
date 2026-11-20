use std::io::{Read, Write};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};

pub fn read<T: DeserializeOwned>(mut reader: impl Read) -> Result<T> {
    let mut text = Vec::new();
    reader.read_to_end(&mut text)?;
    serde_json::from_slice(&text).map_err(|error| Error::Malformed(error.to_string()))
}

pub fn write<T: Serialize>(value: &T, mut writer: impl Write) -> Result<()> {
    let text = serde_json::to_vec(value).map_err(|error| Error::Malformed(error.to_string()))?;
    writer.write_all(&text)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
