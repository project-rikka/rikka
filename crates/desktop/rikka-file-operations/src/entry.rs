use std::fs::Metadata;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::error::{Error, Result};
use crate::item::{EntryKind, Item, Link};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub kind: EntryKind,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub mode: u32,
    pub link: Option<Link>,
}

impl Entry {
    pub fn read(path: &Path) -> Result<Self, PathBuf> {
        let own = std::fs::symlink_metadata(path)
            .map_err(|error| Error::from_io(&error, &path.to_path_buf()))?;
        let (link, followed) = if own.is_symlink() {
            let target = std::fs::metadata(path).ok();
            let link = Link {
                target: std::fs::read_link(path).unwrap_or_default(),
                broken: target.is_none(),
            };
            (Some(link), target.unwrap_or(own))
        } else {
            (None, own)
        };
        let name = path.file_name().map_or_else(
            || "/".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        Ok(Self {
            path: path.to_path_buf(),
            name,
            kind: kind_of(&followed),
            size: if followed.is_dir() { 0 } else { followed.len() },
            modified: followed.modified().ok(),
            created: followed.created().ok(),
            mode: followed.mode(),
            link,
        })
    }

    #[must_use]
    pub fn is_directory(&self) -> bool {
        self.kind.is_directory()
    }
}

impl Item for Entry {
    type Location = PathBuf;

    fn location(&self) -> &PathBuf {
        &self.path
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn kind(&self) -> EntryKind {
        self.kind
    }

    fn link(&self) -> Option<&Link> {
        self.link.as_ref()
    }

    fn size(&self) -> u64 {
        self.size
    }
}

fn kind_of(metadata: &Metadata) -> EntryKind {
    let kind = metadata.file_type();
    if kind.is_dir() {
        EntryKind::Directory
    } else if kind.is_file() {
        EntryKind::File
    } else {
        EntryKind::Special
    }
}
