use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use files_core::{Entry, Location, SCHEME_FILE, SCHEME_TRASH};
use rikka_file_operations::{Action, Local, Sink, Store};
use rikka_mime::Database;

use crate::backend::{Backend, Capabilities, Detail, ListOptions};
use crate::error::{Error, Result};
use crate::stat;
use crate::watch::{Watch, WatchSink};

#[derive(Debug)]
pub struct LocalBackend {
    mime: Arc<Database>,
}

impl LocalBackend {
    #[must_use]
    pub fn new(mime: Arc<Database>) -> Self {
        Self { mime }
    }

    fn path_of(at: &Location) -> Result<PathBuf> {
        at.to_path()
            .ok_or_else(|| Error::NoBackend { at: at.clone() })
    }

    fn reading(&self) -> stat::Reading<'_> {
        stat::Reading {
            mime: &self.mime,
            scheme: SCHEME_FILE,
            detail: Detail::Typed,
        }
    }
}

fn located(error: rikka_file_operations::Error<PathBuf>) -> Error {
    error.map(Location::from_path)
}

impl Store for LocalBackend {
    type Location = Location;
    type Item = Entry;

    fn stat(&self, at: &Location) -> Result<Entry> {
        let path = Self::path_of(at)?;
        let name = path.file_name().map_or_else(
            || "/".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let hidden = name.starts_with('.');
        stat::entry(&self.reading(), &path, name, hidden)
    }

    fn walk(&self, at: &Location) -> Result<Vec<Entry>> {
        self.list(at, ListOptions::walk())
    }

    fn create_directory(&self, at: &Location) -> Result<()> {
        Local.create_directory(&Self::path_of(at)?).map_err(located)
    }

    fn delete(&self, at: &Location) -> Result<()> {
        Local.delete(&Self::path_of(at)?).map_err(located)
    }

    fn trash(&self, at: &Location) -> Result<Location> {
        Local
            .trash(&Self::path_of(at)?)
            .map(|trashed| Location::from_scheme_and_path(SCHEME_TRASH, trashed))
            .map_err(located)
    }

    fn restore(&self, at: &Location) -> Result<Location> {
        Err(Error::Unsupported {
            at: at.clone(),
            action: Action::Restore,
        })
    }

    fn read(&self, at: &Location) -> Result<Box<dyn Read + Send>> {
        Local.read(&Self::path_of(at)?).map_err(located)
    }

    fn write(&self, at: &Location) -> Result<Box<dyn Sink>> {
        Local.write(&Self::path_of(at)?).map_err(located)
    }

    fn link(&self, target: &Path, at: &Location) -> Result<()> {
        Local.link(target, &Self::path_of(at)?).map_err(located)
    }

    fn copy_metadata(&self, from: &Entry, to: &Location) -> Result<()> {
        Local
            .set_metadata(&Self::path_of(to)?, from.mode, from.modified)
            .map_err(located)
    }

    fn can_rename_across(&self, from: &Location, to: &Location) -> bool {
        match (from.to_path(), to.to_path()) {
            (Some(from), Some(to)) => Local.can_rename_across(&from, &to),
            _ => false,
        }
    }

    fn rename_across(&self, from: &Location, to: &Location) -> Result<()> {
        Local
            .rename_across(&Self::path_of(from)?, &Self::path_of(to)?)
            .map_err(located)
    }
}

impl Backend for LocalBackend {
    fn scheme(&self) -> &str {
        SCHEME_FILE
    }

    fn list(&self, at: &Location, options: ListOptions) -> Result<Vec<Entry>> {
        stat::list(&self.mime, SCHEME_FILE, &Self::path_of(at)?, at, options)
    }

    fn capabilities(&self, at: &Location) -> Capabilities {
        let Ok(path) = Self::path_of(at) else {
            return Capabilities::read_only();
        };
        let directory = if path.is_dir() {
            path.clone()
        } else {
            path.parent().unwrap_or(&path).to_path_buf()
        };
        let writable = Local.is_writable(&directory);
        Capabilities {
            create: writable,
            rename: writable,
            delete: writable,
            trash: writable && rikka_trash::can_trash(&path),
            restore: false,
            empty: false,
            watch: true,
            write: writable,
        }
    }

    fn create_file(&self, at: &Location) -> Result<()> {
        Local.create_file(&Self::path_of(at)?).map_err(located)
    }

    fn rename(&self, at: &Location, name: &str) -> Result<Location> {
        Local
            .rename(&Self::path_of(at)?, name)
            .map(Location::from_path)
            .map_err(located)
    }

    fn watch(&self, at: &Location, sink: WatchSink) -> Result<Watch> {
        Watch::directory(&Self::path_of(at)?, at, sink)
    }
}
