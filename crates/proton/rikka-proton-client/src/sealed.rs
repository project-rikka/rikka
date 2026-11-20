use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rikka_proton_api::{Session, SessionStore, private_file};
use rikka_proton_pgp::SymmetricKey;
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Clone)]
pub struct SealedFile {
    path: PathBuf,
    key: SymmetricKey,
    purpose: Vec<u8>,
}

impl SealedFile {
    #[must_use]
    pub fn new(
        path: impl Into<PathBuf>,
        key: SymmetricKey,
        purpose: impl Into<Vec<u8>>,
    ) -> SealedFile {
        SealedFile {
            path: path.into(),
            key,
            purpose: purpose.into(),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read<T: DeserializeOwned>(&self) -> io::Result<Option<T>> {
        let sealed = match fs::read(&self.path) {
            Ok(sealed) => sealed,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let plain = self
            .key
            .open(&sealed, &self.purpose)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        serde_json::from_slice(&plain)
            .map(Some)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn write<T: Serialize>(&self, value: &T) -> io::Result<()> {
        let plain = serde_json::to_vec(value)?;
        let sealed = self
            .key
            .seal(&plain, &self.purpose)
            .map_err(io::Error::other)?;
        if let Some(dir) = self.path.parent() {
            private_file::create_private_dir(dir)?;
        }
        private_file::write(&self.path, &sealed)
    }

    pub fn remove(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}

impl fmt::Debug for SealedFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SealedFile")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct SealedSessionStore(SealedFile);

impl SealedSessionStore {
    #[must_use]
    pub fn new(sealed: SealedFile) -> SealedSessionStore {
        SealedSessionStore(sealed)
    }
}

impl SessionStore for SealedSessionStore {
    fn load(&self) -> rikka_proton_api::Result<Option<Session>> {
        match self.0.read() {
            Ok(session) => Ok(session),
            Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                tracing::warn!(path = %self.0.path().display(), %error, "discarding unreadable stored session");
                Ok(None)
            }
            Err(error) => Err(rikka_proton_api::Error::Storage(error)),
        }
    }

    fn save(&self, session: &Session) -> rikka_proton_api::Result<()> {
        self.0
            .write(session)
            .map_err(rikka_proton_api::Error::Storage)
    }

    fn remove(&self) -> rikka_proton_api::Result<()> {
        self.0.remove().map_err(rikka_proton_api::Error::Storage)
    }
}
