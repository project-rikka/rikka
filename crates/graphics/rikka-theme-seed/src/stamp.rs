use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Stamp {
    pub(crate) modified: i64,
    pub(crate) len: u64,
}

impl Stamp {
    pub(crate) fn from_path(path: &Path) -> Result<Stamp> {
        let metadata = std::fs::metadata(path).map_err(|source| Error::Read {
            path: path.to_path_buf(),
            source,
        })?;
        if !metadata.is_file() {
            return Err(Error::Read {
                path: path.to_path_buf(),
                source: std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a file"),
            });
        }
        Ok(Stamp {
            modified: metadata.modified().map_or(0, nanos),
            len: metadata.len(),
        })
    }
}

fn nanos(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_nanos()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_nanos()).map_or(i64::MIN, |n| -n),
    }
}
