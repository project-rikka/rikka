use std::path::PathBuf;
use std::time::SystemTime;

use crate::compression::Compression;
use crate::error::Result;
use crate::path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    HardLink,
    Special,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub(crate) name: String,
    pub(crate) kind: EntryKind,
    pub(crate) link_target: Option<String>,
    pub(crate) size: u64,
    pub(crate) compressed_size: Option<u64>,
    pub(crate) modified: Option<SystemTime>,
    pub(crate) mode: Option<u32>,
    pub(crate) crc32: Option<u32>,
    pub(crate) encrypted: bool,
    pub(crate) compression: Option<Compression>,
}

impl Entry {
    pub(crate) fn new(name: String, kind: EntryKind) -> Entry {
        Entry {
            name,
            kind,
            link_target: None,
            size: 0,
            compressed_size: None,
            modified: None,
            mode: None,
            crc32: None,
            encrypted: false,
            compression: None,
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn relative_path(&self) -> Result<PathBuf> {
        path::relative(&self.name)
    }

    #[must_use]
    pub fn file_name(&self) -> &str {
        self.name
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or(&self.name)
    }

    #[must_use]
    pub fn kind(&self) -> EntryKind {
        self.kind
    }

    #[must_use]
    pub fn is_file(&self) -> bool {
        matches!(self.kind, EntryKind::File)
    }

    #[must_use]
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, EntryKind::Directory)
    }

    #[must_use]
    pub fn is_symlink(&self) -> bool {
        matches!(self.kind, EntryKind::Symlink)
    }

    #[must_use]
    pub fn link_target(&self) -> Option<&str> {
        self.link_target.as_deref()
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub fn compressed_size(&self) -> Option<u64> {
        self.compressed_size
    }

    #[must_use]
    pub fn modified(&self) -> Option<SystemTime> {
        self.modified
    }

    #[must_use]
    pub fn mode(&self) -> Option<u32> {
        self.mode
    }

    #[must_use]
    pub fn crc32(&self) -> Option<u32> {
        self.crc32
    }

    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.encrypted
    }

    #[must_use]
    pub fn compression(&self) -> Option<Compression> {
        self.compression
    }
}
