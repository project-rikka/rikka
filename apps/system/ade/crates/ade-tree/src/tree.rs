use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ade_watch::Watch;

use crate::entry::Entry;
use crate::rebase::rebased;
use crate::row::Row;

#[derive(Debug)]
pub struct Tree {
    root: PathBuf,
    read: BTreeMap<PathBuf, Vec<Entry>>,
    open: BTreeSet<PathBuf>,
    watch: Watch,
}

impl Tree {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, watch: Watch) -> Tree {
        Tree {
            root: root.into(),
            read: BTreeMap::new(),
            open: BTreeSet::new(),
            watch,
        }
    }

    pub fn opened(&mut self, folder: PathBuf, entries: Vec<Entry>) {
        self.read.insert(folder, entries);
        self.watch.set_folders(self.read.keys().cloned().collect());
    }

    #[must_use]
    pub fn holds(&self, folder: &Path) -> bool {
        self.read.contains_key(folder)
    }

    #[must_use]
    pub fn knows(&self, path: &Path) -> bool {
        path == self.root || self.entry(path).is_some()
    }

    #[must_use]
    pub fn is_folder(&self, path: &Path) -> bool {
        path == self.root || self.entry(path).is_some_and(Entry::is_folder)
    }

    fn entry(&self, path: &Path) -> Option<&Entry> {
        let name = path.file_name()?.to_string_lossy();
        self.read
            .get(path.parent()?)?
            .iter()
            .find(|entry| entry.name() == name)
    }

    #[must_use]
    pub fn toggle(&mut self, folder: &Path) -> Option<PathBuf> {
        if self.open.remove(folder) {
            return None;
        }
        self.open.insert(folder.to_owned());
        (!self.read.contains_key(folder)).then(|| folder.to_owned())
    }

    #[must_use]
    pub fn reveal(&mut self, path: &Path) -> Vec<PathBuf> {
        let mut unread = Vec::new();
        let Ok(under) = path.strip_prefix(&self.root) else {
            return unread;
        };
        let mut folder = self.root.clone();
        let steps: Vec<_> = under.components().collect();
        for step in steps.iter().take(steps.len().saturating_sub(1)) {
            folder.push(step);
            self.open.insert(folder.clone());
            if !self.read.contains_key(&folder) {
                unread.push(folder.clone());
            }
        }
        unread
    }

    #[must_use]
    pub fn open_up(&mut self, folder: &Path) -> Option<PathBuf> {
        if folder == self.root || self.open.contains(folder) {
            return None;
        }
        self.toggle(folder)
    }

    pub fn collapse_all(&mut self) {
        self.open.clear();
    }

    pub fn moved(&mut self, from: &Path, to: &Path) {
        self.open = self
            .open
            .iter()
            .map(|path| rebased(path, from, to).unwrap_or_else(|| path.clone()))
            .collect();
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn is_read(&self) -> bool {
        self.read.contains_key(&self.root)
    }

    #[must_use]
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = Vec::new();
        self.walk(&self.root.clone(), 0, &mut rows);
        rows
    }

    fn walk(&self, folder: &Path, depth: usize, rows: &mut Vec<Row>) {
        let Some(entries) = self.read.get(folder) else {
            return;
        };
        for entry in entries {
            let path = folder.join(entry.name());
            let open = entry.is_folder() && self.open.contains(&path);
            rows.push(Row::new(
                path.clone(),
                entry.name().to_owned(),
                depth,
                entry.is_folder(),
                open,
            ));
            if open {
                self.walk(&path, depth + 1, rows);
            }
        }
    }
}

#[must_use]
pub fn read(folder: &Path) -> Vec<Entry> {
    let listing = match std::fs::read_dir(folder) {
        Ok(listing) => listing,
        Err(error) => {
            tracing::warn!(%error, path = %folder.display(), "failed to read folder");
            return Vec::new();
        }
    };
    let mut entries: Vec<Entry> = listing
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let folder = match entry.file_type() {
                Ok(kind) if kind.is_symlink() => entry.path().is_dir(),
                Ok(kind) => kind.is_dir(),
                Err(_) => false,
            };
            Some(Entry::new(name, folder))
        })
        .collect();
    entries.sort_by_cached_key(|entry| {
        (
            !entry.is_folder(),
            entry.name().to_lowercase(),
            entry.name().to_owned(),
        )
    });
    entries
}
