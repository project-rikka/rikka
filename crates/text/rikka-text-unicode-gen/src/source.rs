use std::fs;
use std::path::PathBuf;

use crate::error::{Error, Result};

#[derive(Debug)]
pub(crate) struct Directory {
    root: PathBuf,
}

impl Directory {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn read(&self, name: &str) -> Result<String> {
        let path = self.root.join(name);
        fs::read_to_string(&path).map_err(|source| Error::Read { path, source })
    }
}
