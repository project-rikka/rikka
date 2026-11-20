use std::io;
use std::path::{Path, PathBuf};

use crate::files::{read_if_exists, remove_if_exists};

#[derive(Debug)]
pub(crate) struct FlakeLock {
    path: PathBuf,
    original: Option<String>,
}

impl FlakeLock {
    pub(crate) fn open(flake: &Path) -> io::Result<Self> {
        let path = flake.join("flake.lock");
        let original = read_if_exists(&path)?;
        Ok(Self { path, original })
    }

    pub(crate) fn restore(&self) -> io::Result<()> {
        match &self.original {
            Some(contents) => rikka_fs::replace(&self.path, contents),
            None => remove_if_exists(&self.path),
        }
    }
}
