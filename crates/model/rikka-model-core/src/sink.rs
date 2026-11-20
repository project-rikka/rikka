use rikka_sync::{Mutex, MutexGuard};
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use rustc_hash::FxHashMap;

use crate::error::{Error, Result};
use crate::path::{contained, key, relative_parts};
use crate::unfollowed::{self, Refusal};

pub trait Sink: Send + Sync {
    fn write(&self, uri: &str, bytes: &[u8]) -> Result<()>;
}

#[derive(Debug)]
pub struct DirectorySink {
    root: PathBuf,
}

impl DirectorySink {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn place(&self, uri: &str) -> Result<PathBuf> {
        let parts = relative_parts(uri).ok_or_else(|| outside(uri))?;
        let (name, folders) = parts
            .split_last()
            .ok_or_else(|| Error::write(uri, "resource has no file name"))?;

        fs::create_dir_all(&self.root).map_err(|error| Error::write(uri, error.to_string()))?;
        let mut folder = contained(&self.root, &self.root).ok_or_else(|| outside(uri))?;
        for part in folders {
            let next = folder.join(part);
            match fs::create_dir(&next) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(Error::write(uri, error.to_string())),
            }
            folder = contained(&self.root, &next).ok_or_else(|| outside(uri))?;
        }
        Ok(folder.join(name))
    }
}

impl Sink for DirectorySink {
    fn write(&self, uri: &str, bytes: &[u8]) -> Result<()> {
        let path = self.place(uri)?;
        let mut file = unfollowed::create(&path).map_err(|refusal| match refusal {
            Refusal::Link => outside(uri),
            Refusal::Io(error) => Error::write(uri, error.to_string()),
        })?;
        file.write_all(bytes)
            .map_err(|error| Error::write(uri, error.to_string()))
    }
}

fn outside(uri: &str) -> Error {
    Error::write(uri, "path escapes model directory")
}

#[derive(Debug, Default)]
pub struct MemorySink {
    files: Mutex<FxHashMap<String, Vec<u8>>>,
}

impl MemorySink {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, uri: &str) -> Option<Vec<u8>> {
        self.files().get(&key(uri)).cloned()
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.files().keys().cloned().collect();
        names.sort();
        names
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files().is_empty()
    }

    fn files(&self) -> MutexGuard<'_, FxHashMap<String, Vec<u8>>> {
        self.files.lock()
    }
}

impl Sink for MemorySink {
    fn write(&self, uri: &str, bytes: &[u8]) -> Result<()> {
        if relative_parts(uri).is_none_or(|parts| parts.is_empty()) {
            return Err(Error::write(uri, "resource has no file name"));
        }
        self.files().insert(key(uri), bytes.to_vec());
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct NoSink;

impl Sink for NoSink {
    fn write(&self, uri: &str, _bytes: &[u8]) -> Result<()> {
        Err(Error::write(
            uri,
            "export has no resource sink; embed resources instead",
        ))
    }
}
