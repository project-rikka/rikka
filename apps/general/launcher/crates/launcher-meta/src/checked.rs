use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer};

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Sha1(String);

impl Sha1 {
    pub fn new(digest: &str) -> Result<Sha1> {
        let valid = digest.len() == 40 && digest.bytes().all(|byte| byte.is_ascii_hexdigit());
        match valid {
            true => Ok(Sha1(digest.to_ascii_lowercase())),
            false => Err(Error::Digest(digest.to_owned())),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Sha1 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Sha1, D::Error> {
        let digest = String::deserialize(deserializer)?;
        Sha1::new(&digest).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RelativePath(String);

impl RelativePath {
    pub fn new(path: &str) -> Result<RelativePath> {
        let safe = !path.is_empty()
            && !path.contains('\\')
            && !path.contains(':')
            && !path.contains('\0')
            && path
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != "..");
        match safe {
            true => Ok(RelativePath(path.to_owned())),
            false => Err(Error::Path(path.to_owned())),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn under(&self, root: &Path) -> PathBuf {
        self.0
            .split('/')
            .fold(root.to_path_buf(), |path, part| path.join(part))
    }

    #[must_use]
    pub fn parent(&self) -> Option<RelativePath> {
        self.0
            .rsplit_once('/')
            .map(|(parent, _)| RelativePath(parent.to_owned()))
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RelativePath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<RelativePath, D::Error> {
        let path = String::deserialize(deserializer)?;
        RelativePath::new(&path).map_err(serde::de::Error::custom)
    }
}
