use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{DecodeError, Error, Result};
use crate::version::VERSION;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct JsonFormat {
    name: &'static str,
}

#[derive(Serialize)]
struct Written<'a, T: ?Sized> {
    format: &'static str,
    version: u32,
    content: &'a T,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored<T> {
    format: String,
    version: u32,
    content: T,
}

#[derive(Deserialize)]
struct Header {
    format: Option<String>,
    version: Option<u32>,
}

impl JsonFormat {
    #[must_use]
    pub const fn new(name: &'static str) -> JsonFormat {
        JsonFormat { name }
    }

    pub fn encode<T: Serialize + ?Sized>(&self, content: &T) -> Result<String> {
        let written = Written {
            format: self.name,
            version: VERSION,
            content,
        };
        let mut json = serde_json::to_string_pretty(&written).map_err(|source| Error::Encode {
            format: self.name,
            source,
        })?;
        json.push('\n');
        Ok(json)
    }

    pub fn decode<T: DeserializeOwned>(&self, json: impl AsRef<[u8]>) -> Result<T> {
        let json = json.as_ref();
        match serde_json::from_slice::<Stored<T>>(json) {
            Ok(stored) => {
                self.identify(Some(&stored.format), Some(stored.version))?;
                Ok(stored.content)
            }
            Err(source) => {
                if let Ok(header) = serde_json::from_slice::<Header>(json) {
                    self.identify(header.format.as_deref(), header.version)?;
                }
                Err(Error::Json {
                    format: self.name,
                    source,
                })
            }
        }
    }

    pub fn save<T: Serialize + ?Sized>(&self, path: impl AsRef<Path>, content: &T) -> Result<()> {
        let path = path.as_ref();
        let json = self.encode(content)?;
        rikka_fs::replace(path, json).map_err(|source| Error::Write {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn load<T: DeserializeOwned>(&self, path: impl AsRef<Path>) -> Result<T> {
        let path = path.as_ref();
        let json = std::fs::read(path).map_err(|source| Error::Read {
            path: path.to_path_buf(),
            source,
        })?;
        self.decode(json)
    }

    fn identify(&self, format: Option<&str>, version: Option<u32>) -> Result<(), DecodeError> {
        if format != Some(self.name) {
            return Err(DecodeError::Foreign {
                expected: self.name,
            });
        }
        match version {
            Some(found) if found != VERSION => Err(DecodeError::Version {
                format: self.name,
                found,
            }),
            _ => Ok(()),
        }
    }
}
