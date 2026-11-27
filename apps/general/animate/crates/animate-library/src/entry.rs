use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use crate::details::Details;
use crate::fit::Fit;
use crate::format::Format;
use crate::hash::Fnv;
use crate::shelf::Shelf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(u64);

impl EntryId {
    #[must_use]
    pub fn from_path(path: &Path) -> EntryId {
        EntryId(Fnv::new().path(path).finish())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub(crate) size: u64,
    pub(crate) modified: SystemTime,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub(crate) id: EntryId,
    pub(crate) path: Arc<Path>,
    pub(crate) shelf: Shelf,
    pub(crate) folder: Arc<str>,
    pub(crate) name: Arc<str>,
    pub(crate) format: Format,
    pub(crate) stamp: Stamp,
    pub(crate) sidecar: Option<Arc<Path>>,
    pub(crate) details: Details,
    pub(crate) fit: Fit,
}

impl Entry {
    #[must_use]
    pub fn id(&self) -> EntryId {
        self.id
    }

    #[must_use]
    pub fn path(&self) -> &Arc<Path> {
        &self.path
    }

    #[must_use]
    pub fn shelf(&self) -> Shelf {
        self.shelf
    }

    #[must_use]
    pub fn folder(&self) -> &Arc<str> {
        &self.folder
    }

    #[must_use]
    pub fn name(&self) -> &Arc<str> {
        &self.name
    }

    #[must_use]
    pub fn format(&self) -> Format {
        self.format
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.stamp.size
    }

    #[must_use]
    pub fn modified(&self) -> SystemTime {
        self.stamp.modified
    }

    #[must_use]
    pub fn sidecar(&self) -> Option<&Arc<Path>> {
        self.sidecar.as_ref()
    }

    #[must_use]
    pub fn details(&self) -> &Details {
        &self.details
    }

    #[must_use]
    pub fn fit(&self) -> Fit {
        self.fit
    }

    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.fit == Fit::Fits
    }

    pub(crate) fn with_details(&self, details: Details) -> Entry {
        Entry {
            fit: Fit::judge(self.shelf, self.format, &details),
            details,
            ..self.clone()
        }
    }
}
