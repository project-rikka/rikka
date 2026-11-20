use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use crate::error::{Error, Result};

const MAX_DOMAIN: usize = 128;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Domain(Arc<str>);

impl Domain {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn file_name(&self) -> String {
        format!("{}.toml", self.as_str())
    }

    #[must_use]
    pub fn is_valid(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= MAX_DOMAIN
            && name.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            })
            && !name.starts_with('.')
            && !name.ends_with('.')
            && !name.contains("..")
    }
}

impl FromStr for Domain {
    type Err = Error;

    fn from_str(name: &str) -> Result<Self> {
        if Self::is_valid(name) {
            Ok(Self(Arc::from(name)))
        } else {
            Err(Error::InvalidDomain(name.to_owned()))
        }
    }
}

impl fmt::Display for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Domain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl serde::Serialize for Domain {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Domain {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}

#[must_use]
pub(crate) fn is_valid_key(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_DOMAIN
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}
