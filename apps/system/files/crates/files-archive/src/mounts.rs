use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use rikka_format::unique_name;

use crate::error::ArchiveError;
use crate::format;
use crate::tree::Tree;

#[derive(Debug)]
pub(crate) struct Mount {
    pub(crate) archive: PathBuf,
    pub(crate) password: Option<String>,
    pub(crate) tree: Arc<Tree>,
    stamp: Option<Stamp>,
}

#[derive(Debug, Default)]
pub(crate) struct Mounts {
    open: HashMap<String, Mount>,
}

impl Mounts {
    pub(crate) fn open(
        &mut self,
        archive: &Path,
        password: Option<&str>,
    ) -> Result<String, ArchiveError> {
        if let Some(name) = self.named(archive) {
            if password.is_none() {
                self.refresh(&name)?;
                return Ok(name);
            }
            self.open.remove(&name);
        }
        let mount = Self::read(archive, password.map(str::to_owned))?;
        let name = self.free_name(archive);
        self.open.insert(name.clone(), mount);
        Ok(name)
    }

    pub(crate) fn mount(&mut self, name: &str) -> Option<&Mount> {
        self.refresh(name).ok()?;
        self.open.get(name)
    }

    pub(crate) fn archive_of(&self, name: &str) -> Option<&Path> {
        self.open.get(name).map(|mount| mount.archive.as_path())
    }

    fn named(&self, archive: &Path) -> Option<String> {
        self.open
            .iter()
            .find(|(_, mount)| mount.archive == archive)
            .map(|(name, _)| name.clone())
    }

    fn refresh(&mut self, name: &str) -> Result<(), ArchiveError> {
        let Some(mount) = self.open.get(name) else {
            return Ok(());
        };
        if mount.stamp.is_some() && mount.stamp == Stamp::read(&mount.archive) {
            return Ok(());
        }
        let fresh = Self::read(&mount.archive, mount.password.clone())?;
        self.open.insert(name.to_owned(), fresh);
        Ok(())
    }

    fn read(archive: &Path, password: Option<String>) -> Result<Mount, ArchiveError> {
        let stamp = Stamp::read(archive);
        let tree = Tree::read(archive, password.as_deref())?;
        Ok(Mount {
            archive: archive.to_path_buf(),
            password,
            tree: Arc::new(tree),
            stamp,
        })
    }

    fn free_name(&self, archive: &Path) -> String {
        unique_name(&format::file_name(archive), |candidate| {
            self.open.contains_key(candidate)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl Stamp {
    fn read(path: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        Some(Self {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        })
    }
}
