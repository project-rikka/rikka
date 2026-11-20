use std::fmt::Debug;
use std::path::{Path, PathBuf};

use crate::address::Address;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EntryKind {
    Directory,
    #[default]
    File,
    Special,
}

impl EntryKind {
    #[must_use]
    pub fn is_directory(self) -> bool {
        self == Self::Directory
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub target: PathBuf,
    pub broken: bool,
}

pub trait Item: Clone + Debug + PartialEq + Send + Sync + 'static {
    type Location: Address;

    fn location(&self) -> &Self::Location;

    fn name(&self) -> &str;

    fn kind(&self) -> EntryKind;

    fn link(&self) -> Option<&Link>;

    fn size(&self) -> u64;

    fn is_directory(&self) -> bool {
        self.kind().is_directory()
    }

    fn link_target(&self) -> Option<&Path> {
        self.link().map(|link| link.target.as_path())
    }
}
