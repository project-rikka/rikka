use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use files_core::{Entry, Location, SCHEME_TRASH};
use rikka_file_operations::{Action, EntryKind, Local, Sink, Store};
use rikka_mime::{Database, MimeType};

use crate::backend::{Backend, Capabilities, Detail, ListOptions};
use crate::error::{Error, Result};
use crate::stat;
use crate::watch::{Watch, WatchSink};

#[derive(Debug)]
pub struct TrashBackend {
    mime: Arc<Database>,
}

impl TrashBackend {
    #[must_use]
    pub fn new(mime: Arc<Database>) -> Self {
        Self { mime }
    }

    fn is_root(at: &Location) -> bool {
        at.path() == Path::new("/")
    }

    fn path_of(at: &Location) -> Result<PathBuf> {
        if Self::is_root(at) {
            return Err(unsupported(at, Action::Anything));
        }
        let path = at.path();
        if !rikka_trash::holds(&path) {
            return Err(Error::NotFound { at: at.clone() });
        }
        Ok(path)
    }

    fn reading(&self) -> stat::Reading<'_> {
        stat::Reading {
            mime: &self.mime,
            scheme: SCHEME_TRASH,
            detail: Detail::Typed,
        }
    }

    fn root_listing(&self) -> Vec<Entry> {
        rikka_trash::entries()
            .into_iter()
            .filter_map(|item| {
                let name = item.original.file_name()?.to_string_lossy().into_owned();
                let mut entry = stat::entry(&self.reading(), &item.path, name, false).ok()?;
                entry.modified = item.deleted.or(entry.modified);
                Some(entry)
            })
            .collect()
    }
}

fn trashed(error: rikka_file_operations::Error<PathBuf>) -> Error {
    error.map(|path| Location::from_scheme_and_path(SCHEME_TRASH, path))
}

impl Store for TrashBackend {
    type Location = Location;
    type Item = Entry;

    fn stat(&self, at: &Location) -> Result<Entry> {
        if Self::is_root(at) {
            return Ok(Entry {
                location: at.clone(),
                name: at.name(),
                kind: EntryKind::Directory,
                mime: MimeType::well_known(MimeType::DIRECTORY),
                size: 0,
                modified: None,
                created: None,
                mode: 0o555,
                hidden: false,
                link: None,
            });
        }
        let path = Self::path_of(at)?;
        let name = path
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        stat::entry(&self.reading(), &path, name, false)
    }

    fn walk(&self, at: &Location) -> Result<Vec<Entry>> {
        self.list(at, ListOptions::walk())
    }

    fn create_directory(&self, at: &Location) -> Result<()> {
        Err(unsupported(at, Action::Create))
    }

    fn delete(&self, at: &Location) -> Result<()> {
        let path = Self::path_of(at)?;
        let holds_entries = std::fs::symlink_metadata(&path).is_ok_and(|own| own.is_dir())
            && std::fs::read_dir(&path).is_ok_and(|mut entries| entries.next().is_some());
        if holds_entries {
            return Err(Error::NotEmpty { at: at.clone() });
        }
        Local.erase(&path).map_err(trashed)
    }

    fn trash(&self, at: &Location) -> Result<Location> {
        Err(unsupported(at, Action::Trash))
    }

    fn restore(&self, at: &Location) -> Result<Location> {
        Local
            .restore(&Self::path_of(at)?)
            .map(Location::from_path)
            .map_err(|error| error.map(Location::from_path))
    }

    fn read(&self, at: &Location) -> Result<Box<dyn Read + Send>> {
        Local.read(&Self::path_of(at)?).map_err(trashed)
    }

    fn write(&self, at: &Location) -> Result<Box<dyn Sink>> {
        Err(unsupported(at, Action::Write))
    }

    fn copy_metadata(&self, _from: &Entry, _to: &Location) -> Result<()> {
        Ok(())
    }

    fn can_rename_across(&self, _from: &Location, _to: &Location) -> bool {
        false
    }

    fn rename_across(&self, from: &Location, _to: &Location) -> Result<()> {
        Err(unsupported(from, Action::Move))
    }
}

impl Backend for TrashBackend {
    fn scheme(&self) -> &str {
        SCHEME_TRASH
    }

    fn list(&self, at: &Location, options: ListOptions) -> Result<Vec<Entry>> {
        if Self::is_root(at) {
            return Ok(self.root_listing());
        }
        stat::list(&self.mime, SCHEME_TRASH, &Self::path_of(at)?, at, options)
    }

    fn parent_of(&self, at: &Location) -> Option<Location> {
        let root = Location::root_of(SCHEME_TRASH);
        if *at == root {
            return None;
        }
        Some(if rikka_trash::is_top_level(&at.path()) {
            root
        } else {
            at.parent()?
        })
    }

    fn capabilities(&self, at: &Location) -> Capabilities {
        Capabilities {
            create: false,
            rename: false,
            delete: true,
            trash: false,
            restore: !Self::is_root(at) && rikka_trash::is_top_level(&at.path()),
            empty: Self::is_root(at),
            watch: false,
            write: false,
        }
    }

    fn create_file(&self, at: &Location) -> Result<()> {
        Err(unsupported(at, Action::Create))
    }

    fn rename(&self, at: &Location, _name: &str) -> Result<Location> {
        Err(unsupported(at, Action::Rename))
    }

    fn watch(&self, _at: &Location, _sink: WatchSink) -> Result<Watch> {
        Ok(Watch::inert())
    }
}

fn unsupported(at: &Location, action: Action) -> Error {
    Error::Unsupported {
        at: at.clone(),
        action,
    }
}
