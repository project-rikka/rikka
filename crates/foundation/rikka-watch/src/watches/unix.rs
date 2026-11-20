use std::collections::HashMap;
use std::ffi::OsStr;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use rustix::io::Errno;

use crate::error::{Error, Result};
use crate::event::{Change, ChangeKind, Event};
use crate::reach::Reach;
use crate::tree;

const READ_BUFFER: usize = 16 * 1024;

#[derive(Debug)]
pub struct Watches {
    inotify: OwnedFd,
    spots: HashMap<i32, Spot>,
}

#[derive(Debug)]
struct Spot {
    path: PathBuf,
    root: PathBuf,
    reach: Reach,
}

impl Watches {
    pub fn new() -> Result<Watches> {
        let inotify = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)
            .map_err(|error| Error::Open(error.into()))?;
        Ok(Watches {
            inotify,
            spots: HashMap::new(),
        })
    }

    pub fn watch(&mut self, path: &Path, reach: Reach) -> Result<()> {
        self.add(path, path, reach)?;
        if reach == Reach::Tree {
            for folder in tree::walk(path).folders {
                if folder != path {
                    self.add_below(&folder, path);
                }
            }
        }
        Ok(())
    }

    pub fn unwatch(&mut self, path: &Path) {
        let inotify = &self.inotify;
        self.spots.retain(|descriptor, spot| {
            if spot.root != path {
                return true;
            }
            forget(inotify, *descriptor, &spot.path);
            false
        });
    }

    #[must_use]
    pub fn is_watching(&self, path: &Path) -> bool {
        self.spots.values().any(|spot| spot.root == path)
    }

    pub fn read(&mut self) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        let mut grown: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut moved: Vec<PathBuf> = Vec::new();
        let mut buffer = [MaybeUninit::<u8>::uninit(); READ_BUFFER];
        let mut reader = inotify::Reader::new(&self.inotify, &mut buffer);
        loop {
            let event = match reader.next() {
                Ok(event) => event,
                Err(Errno::AGAIN) => break,
                Err(Errno::INTR) => continue,
                Err(error) => return Err(Error::Read(error.into())),
            };
            let flags = event.events();
            if flags.contains(ReadFlags::QUEUE_OVERFLOW) {
                events.push(Event::Lost);
                continue;
            }
            if flags.contains(ReadFlags::IGNORED) {
                self.spots.remove(&event.wd());
                continue;
            }
            let Some(spot) = self.spots.get(&event.wd()) else {
                continue;
            };
            let named = event.file_name().filter(|name| !name.is_empty());
            let path = match named {
                Some(name) => spot.path.join(OsStr::from_bytes(name.to_bytes())),
                None => spot.path.clone(),
            };
            let own = flags.intersects(ReadFlags::DELETE_SELF | ReadFlags::MOVE_SELF);
            if own && spot.path != spot.root {
                continue;
            }
            if spot.reach == Reach::Tree && flags.contains(ReadFlags::ISDIR) {
                if flags.intersects(ReadFlags::CREATE | ReadFlags::MOVED_TO) {
                    grown.push((path.clone(), spot.root.clone()));
                }
                if flags.contains(ReadFlags::MOVED_FROM) {
                    moved.push(path.clone());
                }
            }
            events.extend(kinds(flags, spot.reach).map(|kind| {
                Event::Changed(Change {
                    path: path.clone(),
                    kind,
                })
            }));
        }
        for folder in moved {
            self.forget_below(&folder);
        }
        for (folder, root) in grown {
            let found = tree::walk(&folder);
            for below in &found.folders {
                self.add_below(below, &root);
            }
            let appeared = found
                .folders
                .into_iter()
                .filter(|below| *below != folder)
                .chain(found.files);
            events.extend(appeared.map(|path| {
                Event::Changed(Change {
                    path,
                    kind: ChangeKind::Created,
                })
            }));
        }
        Ok(events)
    }

    fn add(&mut self, path: &Path, root: &Path, reach: Reach) -> Result<()> {
        let descriptor = match inotify::add_watch(&self.inotify, path, flags(reach)) {
            Ok(descriptor) => descriptor,
            Err(Errno::NOENT) => return Err(Error::NotFound(path.to_path_buf())),
            Err(Errno::NOSPC) => return Err(Error::Limit(path.to_path_buf())),
            Err(error) => {
                return Err(Error::Watch {
                    path: path.to_path_buf(),
                    source: error.into(),
                });
            }
        };
        let wider = self
            .spots
            .get(&descriptor)
            .map(|spot| spot.reach)
            .filter(|known| *known > reach);
        if let Some(known) = wider {
            let _ = inotify::add_watch(&self.inotify, path, flags(known));
            return Ok(());
        }
        self.spots.insert(
            descriptor,
            Spot {
                path: path.to_path_buf(),
                root: root.to_path_buf(),
                reach,
            },
        );
        Ok(())
    }

    fn add_below(&mut self, folder: &Path, root: &Path) {
        if let Err(error) = self.add(folder, root, Reach::Tree) {
            tracing::debug!(%error, "failed to watch folder");
        }
    }

    fn forget_below(&mut self, folder: &Path) {
        let inotify = &self.inotify;
        self.spots.retain(|descriptor, spot| {
            if !spot.path.starts_with(folder) {
                return true;
            }
            forget(inotify, *descriptor, &spot.path);
            false
        });
    }
}

impl AsFd for Watches {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inotify.as_fd()
    }
}

fn forget(inotify: &OwnedFd, descriptor: i32, path: &Path) {
    if let Err(error) = inotify::remove_watch(inotify, descriptor)
        && error != Errno::INVAL
    {
        tracing::debug!(%error, path = %path.display(), "failed to stop watching");
    }
}

fn flags(reach: Reach) -> WatchFlags {
    let entries = WatchFlags::CREATE
        | WatchFlags::DELETE
        | WatchFlags::MOVED_FROM
        | WatchFlags::MOVED_TO
        | WatchFlags::DELETE_SELF
        | WatchFlags::MOVE_SELF;
    match reach {
        Reach::Uses => WatchFlags::OPEN | WatchFlags::CLOSE_WRITE | WatchFlags::CLOSE_NOWRITE,
        Reach::Entries => entries,
        Reach::Contents | Reach::Tree => {
            entries | WatchFlags::MODIFY | WatchFlags::CLOSE_WRITE | WatchFlags::ATTRIB
        }
    }
}

fn kinds(flags: ReadFlags, reach: Reach) -> impl Iterator<Item = ChangeKind> {
    let closed = match reach {
        Reach::Uses => ChangeKind::Closed,
        Reach::Entries | Reach::Contents | Reach::Tree => ChangeKind::Written,
    };
    [
        (ReadFlags::CREATE | ReadFlags::MOVED_TO, ChangeKind::Created),
        (
            ReadFlags::DELETE
                | ReadFlags::MOVED_FROM
                | ReadFlags::DELETE_SELF
                | ReadFlags::MOVE_SELF,
            ChangeKind::Removed,
        ),
        (ReadFlags::MODIFY, ChangeKind::Modified),
        (ReadFlags::CLOSE_WRITE, closed),
        (ReadFlags::ATTRIB, ChangeKind::Metadata),
        (ReadFlags::OPEN, ChangeKind::Opened),
        (ReadFlags::CLOSE_NOWRITE, ChangeKind::Closed),
    ]
    .into_iter()
    .filter(move |(mask, _)| flags.intersects(*mask))
    .map(|(_, kind)| kind)
}
