use std::io;
use std::path::{Path, PathBuf};

use crate::files::{read_if_exists, remove_if_exists};
use crate::owner::Owner;

#[derive(Debug)]
pub(crate) struct FlakeLock {
    path: PathBuf,
    owner: Owner,
    original: Option<String>,
}

impl FlakeLock {
    pub(crate) fn open(flake: &Path) -> io::Result<Self> {
        let path = flake.join("flake.lock");
        let owner = Owner::of(flake)?;
        let original = owner.act(|| read_if_exists(&path))?;
        Ok(Self {
            path,
            owner,
            original,
        })
    }

    pub(crate) fn owner(&self) -> Owner {
        self.owner
    }

    pub(crate) fn restore(&self) -> io::Result<()> {
        self.owner.act(|| match &self.original {
            Some(contents) => rikka_fs::replace(&self.path, contents),
            None => remove_if_exists(&self.path),
        })
    }
}
