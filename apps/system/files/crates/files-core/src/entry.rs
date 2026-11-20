use std::time::SystemTime;

use rikka_file_operations::{EntryKind, Item, Link};
use rikka_mime::MimeType;

use crate::family::Family;
use crate::location::Location;

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub location: Location,
    pub name: String,
    pub kind: EntryKind,
    pub mime: MimeType,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub mode: u32,
    pub hidden: bool,
    pub link: Option<Link>,
}

impl Entry {
    #[must_use]
    pub fn family(&self) -> Family {
        if self.kind.is_directory() {
            Family::Folder
        } else {
            Family::from_mime(&self.mime)
        }
    }

    #[must_use]
    pub fn is_directory(&self) -> bool {
        self.kind.is_directory()
    }

    #[must_use]
    pub fn extension(&self) -> Option<String> {
        let dot = self.name.rfind('.').filter(|&dot| dot > 0)?;
        let extension = &self.name[dot + 1..];
        (!extension.is_empty()).then(|| extension.to_ascii_lowercase())
    }

    #[must_use]
    pub fn is_executable(&self) -> bool {
        self.kind == EntryKind::File && self.mode & 0o111 != 0
    }
}

impl Item for Entry {
    type Location = Location;

    fn location(&self) -> &Location {
        &self.location
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
