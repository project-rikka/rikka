use std::fmt;

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AppVersion {
    value: String,
    separator: usize,
}

impl AppVersion {
    pub fn new(client: impl AsRef<str>, version: impl AsRef<str>) -> Result<AppVersion> {
        AppVersion::parse(format!("{}@{}", client.as_ref(), version.as_ref()))
    }

    pub fn parse(value: impl Into<String>) -> Result<AppVersion> {
        let value = value.into();
        let Some(separator) = value.find('@') else {
            return Err(Error::InvalidAppVersion(value));
        };

        if separator == 0 || separator + 1 == value.len() {
            return Err(Error::InvalidAppVersion(value));
        }

        if value.contains(|character: char| character.is_control() || character.is_whitespace()) {
            return Err(Error::InvalidAppVersion(value));
        }

        Ok(AppVersion { value, separator })
    }

    #[must_use]
    pub fn client(&self) -> &str {
        &self.value[..self.separator]
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.value[self.separator + 1..]
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl AsRef<str> for AppVersion {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for AppVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}
