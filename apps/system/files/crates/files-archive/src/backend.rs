use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::collections::hash_map::Entry as Slot;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use files_core::{Entry, Location, SCHEME_ARCHIVE};
use files_vfs::{Backend, Capabilities, Error, ListOptions, ReadSink, Result, Watch, WatchSink};
use rikka_archive::{Archive, Format};
use rikka_file_operations::{Action, EntryKind, Kind, Sink, Store};
use rikka_mime::{Database, MimeType};

use crate::error::ArchiveError;
use crate::mounts::{Mount, Mounts};
use crate::tree::Node;

const FILE_MODE: u32 = 0o644;
const DIRECTORY_MODE: u32 = 0o755;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ArchiveKey {
    path: PathBuf,
    password: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct Requested {
    entry: usize,
    place: usize,
}

#[derive(Debug)]
struct Batch {
    archive: ArchiveKey,
    requested: Vec<Requested>,
}

#[derive(Debug)]
pub struct ArchiveBackend {
    mime: Arc<Database>,
    mounts: Mutex<Mounts>,
}

impl ArchiveBackend {
    #[must_use]
    pub fn new(mime: Arc<Database>) -> Self {
        Self {
            mime,
            mounts: Mutex::new(Mounts::default()),
        }
    }

    fn with_mount<T>(
        &self,
        at: &Location,
        read: impl FnOnce(&Mount, &str, &Path) -> Result<T>,
    ) -> Result<T> {
        let nowhere = || Error::NotFound { at: at.clone() };
        let (name, inside) = split(at).ok_or_else(nowhere)?;
        let mut mounts = self.locked();
        let mount = mounts.mount(&name).ok_or_else(nowhere)?;
        read(mount, &name, &inside)
    }

    fn source(&self, at: &Location) -> Result<(PathBuf, Option<String>, usize)> {
        self.with_mount(at, |mount, _, inside| {
            let index = mount
                .tree
                .node(inside)
                .ok_or_else(|| Error::NotFound { at: at.clone() })?
                .index
                .ok_or_else(|| Error::WrongKind {
                    at: at.clone(),
                    expected: Kind::File,
                })?;
            Ok((mount.archive.clone(), mount.password.clone(), index))
        })
    }

    fn entry_of(&self, at: Location, node: &Node) -> Entry {
        Entry {
            name: node.name.clone(),
            kind: if node.directory {
                EntryKind::Directory
            } else {
                EntryKind::File
            },
            mime: self.type_of(node),
            size: node.size,
            modified: node.modified,
            created: None,
            mode: node.mode.unwrap_or(if node.directory {
                DIRECTORY_MODE
            } else {
                FILE_MODE
            }),
            hidden: node.name.starts_with('.'),
            link: None,
            location: at,
        }
    }

    fn type_of(&self, node: &Node) -> MimeType {
        if node.directory {
            return MimeType::well_known(MimeType::DIRECTORY);
        }
        self.mime
            .for_name(Path::new(&node.name))
            .into_iter()
            .next()
            .unwrap_or_else(|| MimeType::well_known(MimeType::UNKNOWN))
    }

    fn batched(&self, at: &[Location]) -> Result<Vec<Batch>> {
        let mut batched: Vec<Batch> = Vec::new();
        for (place, location) in at.iter().enumerate() {
            let (path, password, index) = self.source(location)?;
            let key = ArchiveKey { path, password };
            let requested = Requested {
                entry: index,
                place,
            };
            match batched.iter_mut().find(|batch| batch.archive == key) {
                Some(batch) => batch.requested.push(requested),
                None => batched.push(Batch {
                    archive: key,
                    requested: vec![requested],
                }),
            }
        }
        Ok(batched)
    }

    fn locked(&self) -> MutexGuard<'_, Mounts> {
        self.mounts.lock()
    }

    fn refuse(at: &Location, action: Action) -> Error {
        Error::Unsupported {
            at: at.clone(),
            action,
        }
    }
}

impl Backend for ArchiveBackend {
    fn scheme(&self) -> &str {
        SCHEME_ARCHIVE
    }

    fn browses(&self, at: &Location) -> bool {
        at.is_local() && Format::split(&at.name()).is_some()
    }

    fn browse(&self, at: &Location, password: Option<&str>) -> Result<Location> {
        let path = at
            .to_path()
            .ok_or_else(|| Error::NoBackend { at: at.clone() })?;
        let name = self
            .locked()
            .open(&path, password)
            .map_err(|error| error.at(at))?;
        Ok(top(&name))
    }

    fn parent_of(&self, at: &Location) -> Option<Location> {
        let (name, inside) = split(at)?;
        if !inside.as_os_str().is_empty() {
            return at.parent();
        }
        let archive = self.locked().archive_of(&name)?.to_path_buf();
        archive.parent().map(Location::from_path)
    }

    fn list(&self, at: &Location, options: ListOptions) -> Result<Vec<Entry>> {
        let (tree, inside) = self.with_mount(at, |mount, _, inside| {
            Ok((Arc::clone(&mount.tree), inside.to_path_buf()))
        })?;
        let nodes = tree
            .inside(&inside)
            .ok_or_else(|| Error::NotFound { at: at.clone() })?;
        Ok(nodes
            .iter()
            .filter(|node| options.show_hidden || !node.name.starts_with('.'))
            .map(|node| self.entry_of(at.child(&node.name), node))
            .collect())
    }

    fn capabilities(&self, _at: &Location) -> Capabilities {
        Capabilities::read_only()
    }

    fn create_file(&self, at: &Location) -> Result<()> {
        Err(Self::refuse(at, Action::Create))
    }

    fn rename(&self, at: &Location, _name: &str) -> Result<Location> {
        Err(Self::refuse(at, Action::Rename))
    }

    fn watch(&self, at: &Location, _sink: WatchSink) -> Result<Watch> {
        Err(Self::refuse(at, Action::Anything))
    }
}

impl Store for ArchiveBackend {
    type Location = Location;
    type Item = Entry;

    fn stat(&self, at: &Location) -> Result<Entry> {
        self.with_mount(at, |mount, name, inside| {
            if let Some(node) = mount.tree.node(inside) {
                return Ok(self.entry_of(at.clone(), node));
            }
            if !inside.as_os_str().is_empty() {
                return Err(Error::NotFound { at: at.clone() });
            }
            Ok(Entry {
                name: name.to_owned(),
                kind: EntryKind::Directory,
                mime: MimeType::well_known(MimeType::DIRECTORY),
                size: 0,
                modified: std::fs::metadata(&mount.archive)
                    .ok()
                    .and_then(|meta| meta.modified().ok()),
                created: None,
                mode: DIRECTORY_MODE,
                hidden: false,
                link: None,
                location: at.clone(),
            })
        })
    }

    fn walk(&self, at: &Location) -> Result<Vec<Entry>> {
        self.list(at, ListOptions::walk())
    }

    fn create_directory(&self, at: &Location) -> Result<()> {
        Err(Self::refuse(at, Action::Create))
    }

    fn delete(&self, at: &Location) -> Result<()> {
        Err(Self::refuse(at, Action::Anything))
    }

    fn trash(&self, at: &Location) -> Result<Location> {
        Err(Self::refuse(at, Action::Trash))
    }

    fn restore(&self, at: &Location) -> Result<Location> {
        Err(Self::refuse(at, Action::Restore))
    }

    fn read(&self, at: &Location) -> Result<Box<dyn Read + Send>> {
        let (archive, password, index) = self.source(at)?;
        let mut opened = open(&archive, password.as_deref()).map_err(|error| error.at(at))?;
        let bytes = opened
            .read_entry(index)
            .map_err(|error| ArchiveError::from_archive(&error).at(at))?;
        Ok(Box::new(std::io::Cursor::new(bytes)))
    }

    fn read_each(&self, at: &[Location], take: &mut ReadSink<'_>) -> Result<()> {
        for batch in self.batched(at)? {
            let somewhere = batch
                .requested
                .first()
                .map_or(&at[0], |first| &at[first.place]);
            let mut opened = open(&batch.archive.path, batch.archive.password.as_deref())
                .map_err(|error| error.at(somewhere))?;
            let mut left = batch.requested;
            while !left.is_empty() {
                let mut serving: HashMap<usize, usize> = HashMap::new();
                let mut again: Vec<Requested> = Vec::new();
                for requested in left {
                    match serving.entry(requested.entry) {
                        Slot::Occupied(_) => again.push(requested),
                        Slot::Vacant(place) => {
                            place.insert(requested.place);
                        }
                    }
                }
                let indices: Vec<usize> = serving.keys().copied().collect();
                let mut refused: Option<Error> = None;
                let read = opened.read_each(&indices, &mut |index, reader| {
                    let Some(place) = serving.get(&index) else {
                        return Ok(());
                    };
                    match take(*place, Ok(reader)) {
                        Ok(()) => Ok(()),
                        Err(error) => {
                            refused = Some(error);
                            Err(rikka_archive::Error::Cancelled)
                        }
                    }
                });
                if let Some(error) = refused {
                    return Err(error);
                }
                read.map_err(|error| ArchiveError::from_archive(&error).at(somewhere))?;
                left = again;
            }
        }
        Ok(())
    }

    fn write(&self, at: &Location) -> Result<Box<dyn Sink>> {
        Err(Self::refuse(at, Action::Write))
    }

    fn copy_metadata(&self, _from: &Entry, to: &Location) -> Result<()> {
        Err(Self::refuse(to, Action::Write))
    }

    fn can_rename_across(&self, _from: &Location, _to: &Location) -> bool {
        false
    }

    fn rename_across(&self, from: &Location, _to: &Location) -> Result<()> {
        Err(Self::refuse(from, Action::Move))
    }
}

fn open(path: &Path, password: Option<&str>) -> std::result::Result<Archive, ArchiveError> {
    match password {
        Some(password) => Archive::open_with_password(path, password),
        None => Archive::open(path),
    }
    .map_err(|error| ArchiveError::from_archive(&error))
}

fn split(at: &Location) -> Option<(String, PathBuf)> {
    let path = at.path();
    let mut parts = path.components().filter_map(|part| match part {
        Component::Normal(name) => Some(name),
        _ => None,
    });
    let name = parts.next()?.to_str()?.to_owned();
    Some((name, parts.collect()))
}

fn top(name: &str) -> Location {
    Location::from_scheme_and_path(SCHEME_ARCHIVE, Path::new("/").join(name))
}
