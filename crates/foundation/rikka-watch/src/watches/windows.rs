use std::collections::HashMap;
use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::{OsStrExt as _, OsStringExt as _};
use std::os::windows::io::{
    AsHandle, AsRawHandle as _, BorrowedHandle, FromRawHandle as _, OwnedHandle,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use windows::Win32::Foundation::{
    ERROR_IO_INCOMPLETE, ERROR_NOTIFY_ENUM_DIR, ERROR_OPERATION_ABORTED, HANDLE, WAIT_FAILED,
    WIN32_ERROR,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ACTION, FILE_ACTION_ADDED, FILE_ACTION_MODIFIED, FILE_ACTION_REMOVED,
    FILE_ACTION_RENAMED_NEW_NAME, FILE_ACTION_RENAMED_OLD_NAME, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY, FILE_NOTIFY_CHANGE, FILE_NOTIFY_CHANGE_ATTRIBUTES,
    FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE,
    FILE_NOTIFY_CHANGE_SIZE, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    ReadDirectoryChangesW,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::{
    CreateEventW, INFINITE, ResetEvent, SetEvent, WaitForSingleObject,
};
use windows::core::PCWSTR;

use crate::error::{Error, Result};
use crate::event::{Change, ChangeKind, Event};
use crate::reach::Reach;
use crate::tree;

const BUFFER_WORDS: usize = 16 * 1024;

const RECORD_HEADER: usize = 12;

pub struct Watches {
    ready: Arc<OwnedHandle>,
    spots: HashMap<PathBuf, Box<Spot>>,
}

struct Spot {
    path: PathBuf,
    root: PathBuf,
    only: Option<OsString>,
    reach: Reach,
    ready: Arc<OwnedHandle>,
    directory: OwnedHandle,
    overlapped: OVERLAPPED,
    buffer: Box<[u32]>,
    pending: bool,
}

unsafe impl Send for Spot {}

enum Completion {
    Pending,
    Records(usize),
    Overflow,
    Aborted,
    Gone,
}

impl Watches {
    pub fn new() -> Result<Watches> {
        let event = unsafe { CreateEventW(None, true, false, None) }
            .map_err(|error| Error::Open(error.into()))?;
        Ok(Watches {
            ready: Arc::new(unsafe { OwnedHandle::from_raw_handle(event.0) }),
            spots: HashMap::new(),
        })
    }

    pub fn watch(&mut self, path: &Path, reach: Reach) -> Result<()> {
        if reach == Reach::Uses {
            return Err(Error::Unsupported {
                path: path.to_path_buf(),
                reach,
            });
        }
        if self.spots.get(path).is_some_and(|spot| spot.reach >= reach) {
            return Ok(());
        }
        let metadata = std::fs::metadata(path).map_err(|source| match source.kind() {
            io::ErrorKind::NotFound => Error::NotFound(path.to_path_buf()),
            _ => Error::Watch {
                path: path.to_path_buf(),
                source,
            },
        })?;
        let (folder, only) = match metadata.is_dir() {
            true => (path.to_path_buf(), None),
            false => match (path.parent(), path.file_name()) {
                (Some(parent), Some(name)) => (parent.to_path_buf(), Some(name.to_os_string())),
                _ => return Err(Error::NotFound(path.to_path_buf())),
            },
        };
        let failed = |source: io::Error| Error::Watch {
            path: path.to_path_buf(),
            source,
        };
        let mut spot = Box::new(Spot {
            directory: open_directory(&folder).map_err(failed)?,
            path: folder,
            root: path.to_path_buf(),
            only,
            reach,
            ready: Arc::clone(&self.ready),
            overlapped: OVERLAPPED::default(),
            buffer: vec![0; BUFFER_WORDS].into_boxed_slice(),
            pending: false,
        });
        spot.issue().map_err(failed)?;
        self.spots.insert(path.to_path_buf(), spot);
        Ok(())
    }

    pub fn unwatch(&mut self, path: &Path) {
        self.spots.remove(path);
    }

    #[must_use]
    pub fn is_watching(&self, path: &Path) -> bool {
        self.spots.contains_key(path)
    }

    pub fn read(&mut self) -> Result<Vec<Event>> {
        unsafe { ResetEvent(raw(self.ready.as_handle())) }
            .map_err(|error| Error::Read(error.into()))?;
        let mut events = Vec::new();
        loop {
            let mut progressed = false;
            let mut gone = Vec::new();
            for (root, spot) in &mut self.spots {
                match spot.complete() {
                    Completion::Pending => continue,
                    Completion::Records(length) => spot.records(length, &mut events),
                    Completion::Overflow => events.push(Event::Lost),
                    Completion::Aborted => {}
                    Completion::Gone => {
                        events.push(removed(root.clone()));
                        gone.push(root.clone());
                        continue;
                    }
                }
                progressed = true;
                if let Err(error) = spot.issue() {
                    tracing::debug!(%error, path = %root.display(), "watched folder is gone");
                    events.push(removed(root.clone()));
                    gone.push(root.clone());
                }
            }
            for root in gone {
                self.spots.remove(&root);
            }
            if !progressed {
                return Ok(events);
            }
        }
    }
}

impl AsHandle for Watches {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.ready.as_handle()
    }
}

impl std::fmt::Debug for Watches {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Watches")
            .field("watching", &self.spots.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl Spot {
    fn issue(&mut self) -> io::Result<()> {
        self.overlapped = OVERLAPPED {
            hEvent: raw(self.ready.as_handle()),
            ..OVERLAPPED::default()
        };
        let bytes = u32::try_from(self.buffer.len() * size_of::<u32>()).unwrap_or(u32::MAX);
        unsafe {
            ReadDirectoryChangesW(
                raw(self.directory.as_handle()),
                self.buffer.as_mut_ptr().cast(),
                bytes,
                self.reach == Reach::Tree,
                filter(self.reach),
                None,
                Some(&mut self.overlapped),
                None,
            )
        }
        .map_err(io::Error::from)?;
        self.pending = true;
        Ok(())
    }

    fn complete(&mut self) -> Completion {
        if !self.pending {
            return Completion::Aborted;
        }
        let mut transferred = 0u32;
        let outcome = unsafe {
            GetOverlappedResult(
                raw(self.directory.as_handle()),
                &self.overlapped,
                &mut transferred,
                false,
            )
        };
        let code = outcome.as_ref().err().map(WIN32_ERROR::from_error);
        if code == Some(Some(ERROR_IO_INCOMPLETE)) {
            return Completion::Pending;
        }
        self.pending = false;
        match code {
            None if transferred > 0 => Completion::Records(transferred as usize),
            None if self.path.is_dir() => Completion::Overflow,
            None => Completion::Gone,
            Some(Some(ERROR_NOTIFY_ENUM_DIR)) => Completion::Overflow,
            Some(Some(ERROR_OPERATION_ABORTED)) => Completion::Aborted,
            Some(_) => Completion::Gone,
        }
    }

    fn records(&self, length: usize, events: &mut Vec<Event>) {
        let bytes = unsafe {
            std::slice::from_raw_parts(
                self.buffer.as_ptr().cast::<u8>(),
                length.min(self.buffer.len() * size_of::<u32>()),
            )
        };
        let mut grown = Vec::new();
        for (action, name) in Records::new(bytes) {
            if let Some(only) = &self.only
                && !only.eq_ignore_ascii_case(&name)
            {
                continue;
            }
            let path = match &self.only {
                Some(_) => self.root.clone(),
                None => self.path.join(&name),
            };
            let kinds: &[ChangeKind] = match action {
                FILE_ACTION_ADDED | FILE_ACTION_RENAMED_NEW_NAME => {
                    if self.reach == Reach::Tree && path.is_dir() {
                        grown.push(path.clone());
                    }
                    &[ChangeKind::Created]
                }
                FILE_ACTION_REMOVED | FILE_ACTION_RENAMED_OLD_NAME => &[ChangeKind::Removed],
                FILE_ACTION_MODIFIED if self.reach != Reach::Entries => {
                    &[ChangeKind::Modified, ChangeKind::Written]
                }
                _ => &[],
            };
            events.extend(kinds.iter().map(|kind| {
                Event::Changed(Change {
                    path: path.clone(),
                    kind: *kind,
                })
            }));
        }
        for folder in grown {
            let found = tree::walk(&folder);
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
    }

    fn settle(&mut self) {
        let directory = raw(self.directory.as_handle());
        let ready = raw(self.ready.as_handle());
        if let Err(error) = unsafe { CancelIoEx(directory, Some(&self.overlapped)) } {
            tracing::debug!(%error, path = %self.path.display(), "failed to cancel folder watch");
        }
        loop {
            if !matches!(self.complete(), Completion::Pending) {
                break;
            }
            if let Err(error) = unsafe { ResetEvent(ready) } {
                tracing::warn!(%error, "failed to reset watch event");
            }
            if !matches!(self.complete(), Completion::Pending) {
                break;
            }
            if unsafe { WaitForSingleObject(ready, INFINITE) } == WAIT_FAILED {
                tracing::warn!(error = %io::Error::last_os_error(), "failed to wait for folder watch");
            }
        }
        if let Err(error) = unsafe { SetEvent(ready) } {
            tracing::debug!(%error, "failed to rearm watch event");
        }
    }
}

impl Drop for Spot {
    fn drop(&mut self) {
        if self.pending {
            self.settle();
        }
    }
}

struct Records<'a> {
    bytes: &'a [u8],
    at: Option<usize>,
}

impl<'a> Records<'a> {
    fn new(bytes: &'a [u8]) -> Records<'a> {
        Records { bytes, at: Some(0) }
    }
}

impl Iterator for Records<'_> {
    type Item = (FILE_ACTION, OsString);

    fn next(&mut self) -> Option<Self::Item> {
        let at = self.at?;
        let header = self.bytes.get(at..at + RECORD_HEADER)?;
        let word = |offset: usize| {
            u32::from_le_bytes([
                header[offset],
                header[offset + 1],
                header[offset + 2],
                header[offset + 3],
            ])
        };
        let (next, action, length) = (word(0) as usize, word(4), word(8) as usize);
        let name = self
            .bytes
            .get(at + RECORD_HEADER..at + RECORD_HEADER + length)?;
        let wide: Vec<u16> = name
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        self.at = (next != 0).then_some(at + next);
        Some((FILE_ACTION(action), OsString::from_wide(&wide)))
    }
}

fn open_directory(folder: &Path) -> io::Result<OwnedHandle> {
    let wide: Vec<u16> = folder
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            None,
        )
    }
    .map_err(io::Error::from)?;
    Ok(unsafe { OwnedHandle::from_raw_handle(handle.0) })
}

fn filter(reach: Reach) -> FILE_NOTIFY_CHANGE {
    let entries = FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME;
    match reach {
        Reach::Uses | Reach::Entries => entries,
        Reach::Contents | Reach::Tree => {
            entries
                | FILE_NOTIFY_CHANGE_LAST_WRITE
                | FILE_NOTIFY_CHANGE_SIZE
                | FILE_NOTIFY_CHANGE_ATTRIBUTES
        }
    }
}

fn removed(path: PathBuf) -> Event {
    Event::Changed(Change {
        path,
        kind: ChangeKind::Removed,
    })
}

fn raw(handle: BorrowedHandle<'_>) -> HANDLE {
    HANDLE(handle.as_raw_handle())
}
