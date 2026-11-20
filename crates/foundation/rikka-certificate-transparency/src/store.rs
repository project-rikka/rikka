use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use base64::prelude::{BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::key::LogListKey;
use crate::list::LogList;
use crate::lists::is_newer;

const FILE: &str = "log_list.signed.json";

#[derive(Serialize, Deserialize)]
struct Envelope {
    list: String,
    signature: String,
}

pub(crate) struct Store {
    path: PathBuf,
}

impl Store {
    pub(crate) fn new(dir: &Path) -> Store {
        Store {
            path: dir.join(FILE),
        }
    }

    pub(crate) fn newer_than(
        &self,
        key: &LogListKey,
        current: Option<&LogList>,
    ) -> Option<LogList> {
        match self.read(key) {
            Ok(cached) => cached.filter(|cached| is_newer(cached, current)),
            Err(error) => {
                tracing::warn!(%error, "ignoring cached CT log list");
                None
            }
        }
    }

    pub(crate) fn write(&self, list: &[u8], signature: &[u8]) -> Result<()> {
        let envelope = Envelope {
            list: BASE64_STANDARD.encode(list),
            signature: BASE64_STANDARD.encode(signature),
        };
        let bytes = serde_json::to_vec(&envelope).expect("an envelope of strings serializes");
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|source| self.io_error(source))?;
        }
        rikka_fs::replace(&self.path, bytes).map_err(|source| self.io_error(source))
    }

    fn read(&self, key: &LogListKey) -> Result<Option<LogList>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(self.io_error(source)),
        };
        let envelope: Envelope =
            serde_json::from_slice(&bytes).map_err(|error| self.malformed(&error))?;
        let list = self.decode(&envelope.list)?;
        let signature = self.decode(&envelope.signature)?;
        LogList::from_signed(&list, &signature, key).map(Some)
    }

    fn decode(&self, encoded: &str) -> Result<Vec<u8>> {
        BASE64_STANDARD
            .decode(encoded)
            .map_err(|error| self.malformed(&error))
    }

    fn io_error(&self, source: io::Error) -> Error {
        Error::Cache {
            path: self.path.clone(),
            source,
        }
    }

    fn malformed(&self, error: &dyn std::error::Error) -> Error {
        Error::MalformedCache {
            path: self.path.clone(),
            reason: error.to_string(),
        }
    }
}
