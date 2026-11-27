use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use rikka_format::natural_cmp;

use crate::entry::{Entry, EntryId};
use crate::location::Location;
use crate::shelf::Shelf;

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    shelves: [ShelfEntries; Shelf::ALL.len()],
    index: HashMap<EntryId, (Shelf, usize)>,
    unavailable: Vec<Unavailable>,
}

#[derive(Clone, Debug, Default)]
pub struct ShelfEntries {
    entries: Vec<Arc<Entry>>,
    folders: Vec<Arc<str>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Unavailable {
    pub location: Location,
    pub reason: Arc<str>,
}

impl Catalog {
    pub(crate) fn new(
        entries: impl IntoIterator<Item = Arc<Entry>>,
        unavailable: Vec<Unavailable>,
    ) -> Catalog {
        let mut shelves: [ShelfEntries; Shelf::ALL.len()] = Default::default();
        for entry in entries {
            shelves[entry.shelf.index()].entries.push(entry);
        }
        let mut index = HashMap::new();
        for (shelf, contents) in Shelf::ALL.into_iter().zip(&mut shelves) {
            contents.entries.sort_by(|left, right| order(left, right));
            let mut folders: Vec<Arc<str>> = Vec::new();
            for (position, entry) in contents.entries.iter().enumerate() {
                index.insert(entry.id, (shelf, position));
                let listed = folders.last().is_some_and(|last| *last == entry.folder);
                if !entry.folder.is_empty() && !listed {
                    folders.push(Arc::clone(&entry.folder));
                }
            }
            contents.folders = folders;
        }
        Catalog {
            shelves,
            index,
            unavailable,
        }
    }

    #[must_use]
    pub fn shelf(&self, shelf: Shelf) -> &ShelfEntries {
        &self.shelves[shelf.index()]
    }

    #[must_use]
    pub fn entry(&self, id: EntryId) -> Option<&Arc<Entry>> {
        let (shelf, position) = self.index.get(&id)?;
        self.shelves[shelf.index()].entries.get(*position)
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.shelves
            .iter()
            .flat_map(|shelf| &shelf.entries)
            .filter(|entry| entry.details.is_pending())
            .count()
    }

    #[must_use]
    pub fn unavailable(&self) -> &[Unavailable] {
        &self.unavailable
    }
}

impl ShelfEntries {
    #[must_use]
    pub fn entries(&self) -> &[Arc<Entry>] {
        &self.entries
    }

    #[must_use]
    pub fn folders(&self) -> &[Arc<str>] {
        &self.folders
    }

    pub fn within<'a>(&'a self, folder: &'a str) -> impl Iterator<Item = &'a Arc<Entry>> + 'a {
        self.entries.iter().filter(move |entry| {
            folder.is_empty()
                || entry
                    .folder
                    .strip_prefix(folder)
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
    }
}

fn order(left: &Entry, right: &Entry) -> Ordering {
    natural_cmp(&left.folder, &right.folder)
        .then_with(|| natural_cmp(&left.name, &right.name))
        .then_with(|| left.path.cmp(&right.path))
}
