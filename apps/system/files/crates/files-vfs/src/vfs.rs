use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

use files_core::{Entry, Location};
use rikka_file_operations::{Sink, Store};
use rikka_mime::Database;

use crate::backend::{Backend, Capabilities, ListOptions};
use crate::error::{Error, Result};
use crate::local::LocalBackend;
use crate::operation::ReadSink;
use crate::trash::TrashBackend;
use crate::watch::{Watch, WatchSink};

#[derive(Debug)]
pub struct Vfs {
    backends: HashMap<String, Arc<dyn Backend>>,
}

impl Vfs {
    #[must_use]
    pub fn new(mime: Arc<Database>) -> Self {
        let mut vfs = Self {
            backends: HashMap::new(),
        };
        vfs.register(Arc::new(LocalBackend::new(Arc::clone(&mime))));
        vfs.register(Arc::new(TrashBackend::new(mime)));
        vfs
    }

    pub fn register(&mut self, backend: Arc<dyn Backend>) {
        self.backends.insert(backend.scheme().to_owned(), backend);
    }

    pub fn backend(&self, at: &Location) -> Result<&Arc<dyn Backend>> {
        self.backends
            .get(at.scheme())
            .ok_or_else(|| Error::NoBackend { at: at.clone() })
    }

    #[must_use]
    pub fn knows(&self, scheme: &str) -> bool {
        self.backends.contains_key(scheme)
    }

    #[must_use]
    pub fn browses(&self, at: &Location) -> bool {
        self.browser(at).is_some()
    }

    pub fn browse(&self, at: &Location, password: Option<&str>) -> Result<Location> {
        self.browser(at)
            .ok_or_else(|| Error::NotFound { at: at.clone() })?
            .browse(at, password)
    }

    fn browser(&self, at: &Location) -> Option<&Arc<dyn Backend>> {
        self.backends.values().find(|backend| backend.browses(at))
    }

    pub fn list(&self, at: &Location, options: ListOptions) -> Result<Vec<Entry>> {
        self.backend(at)?.list(at, options)
    }

    #[must_use]
    pub fn capabilities(&self, at: &Location) -> Capabilities {
        self.backend(at).map_or_else(
            |_| Capabilities::read_only(),
            |backend| backend.capabilities(at),
        )
    }

    pub fn create_file(&self, at: &Location) -> Result<()> {
        self.backend(at)?.create_file(at)
    }

    pub fn rename(&self, at: &Location, name: &str) -> Result<Location> {
        self.backend(at)?.rename(at, name)
    }

    pub fn watch(&self, at: &Location, sink: WatchSink) -> Result<Watch> {
        self.backend(at)?.watch(at, sink)
    }

    #[must_use]
    pub fn parent_of(&self, at: &Location) -> Option<Location> {
        self.backend(at)
            .map_or_else(|_| at.parent(), |backend| backend.parent_of(at))
    }
}

impl Store for Vfs {
    type Location = Location;
    type Item = Entry;

    fn stat(&self, at: &Location) -> Result<Entry> {
        self.backend(at)?.stat(at)
    }

    fn walk(&self, at: &Location) -> Result<Vec<Entry>> {
        self.backend(at)?.walk(at)
    }

    fn create_directory(&self, at: &Location) -> Result<()> {
        self.backend(at)?.create_directory(at)
    }

    fn delete(&self, at: &Location) -> Result<()> {
        self.backend(at)?.delete(at)
    }

    fn trash(&self, at: &Location) -> Result<Location> {
        self.backend(at)?.trash(at)
    }

    fn restore(&self, at: &Location) -> Result<Location> {
        self.backend(at)?.restore(at)
    }

    fn read(&self, at: &Location) -> Result<Box<dyn Read + Send>> {
        self.backend(at)?.read(at)
    }

    fn read_each(&self, at: &[Location], take: &mut ReadSink<'_>) -> Result<()> {
        match at.first() {
            Some(first) => self.backend(first)?.read_each(at, take),
            None => Ok(()),
        }
    }

    fn write(&self, at: &Location) -> Result<Box<dyn Sink>> {
        self.backend(at)?.write(at)
    }

    fn link(&self, target: &Path, at: &Location) -> Result<()> {
        self.backend(at)?.link(target, at)
    }

    fn copy_metadata(&self, from: &Entry, to: &Location) -> Result<()> {
        self.backend(to)?.copy_metadata(from, to)
    }

    fn can_rename_across(&self, from: &Location, to: &Location) -> bool {
        from.scheme() == to.scheme()
            && self
                .backend(from)
                .is_ok_and(|backend| backend.can_rename_across(from, to))
    }

    fn rename_across(&self, from: &Location, to: &Location) -> Result<()> {
        self.backend(from)?.rename_across(from, to)
    }
}
