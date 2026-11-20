use std::io;
use std::path::{Path, PathBuf};

use crate::index::format::{IndexContents, decode, encode};

const INDEX_FILE: &str = "index";

#[derive(Clone, Debug)]
pub(crate) struct IndexStore {
    path: PathBuf,
}

impl IndexStore {
    pub(crate) fn new(dir: &Path) -> Self {
        Self {
            path: dir.join(INDEX_FILE),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn load(&self) -> Option<IndexContents> {
        decode(&std::fs::read(&self.path).ok()?)
    }

    pub(crate) fn save(&self, contents: &IndexContents) -> io::Result<()> {
        rikka_fs::replace(&self.path, encode(contents))
    }
}
