use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use music_core::{Error, Result};
use rikka_audio_decode::StreamReader;

const ABANDONED: &str = "the download was abandoned";

pub struct Spool {
    writer: File,
    reader: File,
    path: PathBuf,
    shared: Arc<Shared>,
}

pub struct Stored {
    file: File,
    position: u64,
    shared: Arc<Shared>,
}

#[derive(Debug, Default)]
struct Shared {
    progress: Mutex<Progress>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct Progress {
    total: Option<u64>,
    received: u64,
    ending: Option<Ending>,
}

#[derive(Clone, Debug)]
enum Ending {
    Complete,
    Broken(String),
}

impl Spool {
    pub fn create(path: &Path) -> Result<Spool> {
        let writer = File::create(path).map_err(disk)?;
        let reader = File::open(path).map_err(disk)?;
        Ok(Spool {
            writer,
            reader,
            path: path.to_owned(),
            shared: Arc::default(),
        })
    }

    pub fn begin(&mut self, total: Option<u64>) {
        self.shared.change(|progress| progress.total = total);
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes).map_err(disk)?;
        let length = bytes.len() as u64;
        self.shared.change(|progress| progress.received += length);
        Ok(())
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub fn stored(&self) -> Result<Stored> {
        Ok(Stored {
            file: self.reader.try_clone().map_err(disk)?,
            position: 0,
            shared: Arc::clone(&self.shared),
        })
    }

    pub(crate) fn finish(&mut self) -> Result<()> {
        self.writer.flush().map_err(disk)?;
        let short = {
            let progress = self.shared.progress();
            progress
                .total
                .filter(|total| *total != progress.received)
                .map(|total| (total, progress.received))
        };
        match short {
            Some((total, received)) => {
                let reason = format!("download ended after {received} of {total} bytes");
                self.end(Ending::Broken(reason.clone()));
                Err(Error::Unreadable(reason))
            }
            None => {
                self.end(Ending::Complete);
                Ok(())
            }
        }
    }

    pub(crate) fn fail(&mut self, reason: &str) {
        self.end(Ending::Broken(reason.to_owned()));
    }

    fn end(&mut self, ending: Ending) {
        self.shared.change(|progress| {
            progress.ending.get_or_insert(ending);
        });
    }
}

impl Drop for Spool {
    fn drop(&mut self) {
        self.end(Ending::Broken(ABANDONED.into()));
    }
}

impl std::fmt::Debug for Spool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Spool")
            .field("path", &self.path)
            .field("progress", &*self.shared.progress())
            .finish_non_exhaustive()
    }
}

impl Stored {
    pub fn whole(path: &Path) -> Result<Stored> {
        let file = File::open(path).map_err(disk)?;
        let total = file.metadata().map_err(disk)?.len();
        Ok(Stored {
            file,
            position: 0,
            shared: Arc::new(Shared {
                progress: Mutex::new(Progress {
                    total: Some(total),
                    received: total,
                    ending: Some(Ending::Complete),
                }),
                changed: Condvar::new(),
            }),
        })
    }

    fn length(&self) -> io::Result<u64> {
        let progress = self
            .shared
            .until(|progress| progress.total.is_some() || progress.ending.is_some());
        match (&progress.ending, progress.total) {
            (_, Some(total)) => Ok(total),
            (Some(Ending::Complete), None) => Ok(progress.received),
            (Some(Ending::Broken(reason)), None) => Err(io::Error::other(reason.clone())),
            (None, None) => Err(io::Error::other(ABANDONED)),
        }
    }
}

impl Read for Stored {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let position = self.position;
        let available = {
            let progress = self
                .shared
                .until(|progress| progress.received > position || progress.ending.is_some());
            if progress.received <= position {
                return match &progress.ending {
                    Some(Ending::Broken(reason)) => Err(io::Error::other(reason.clone())),
                    _ => Ok(0),
                };
            }
            progress.received - position
        };
        let len = buf
            .len()
            .min(usize::try_from(available).unwrap_or(usize::MAX));
        let read = self.file.read_at(&mut buf[..len], position)?;
        self.position += read as u64;
        Ok(read)
    }
}

impl Seek for Stored {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let target = match to {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self.length()?.checked_add_signed(delta),
        };
        let target = target.ok_or_else(|| io::Error::other("a seek before the start"))?;
        self.position = target;
        Ok(target)
    }
}

impl StreamReader for Stored {
    fn complete_len(&self) -> Option<u64> {
        let progress = self.shared.progress();
        matches!(progress.ending, Some(Ending::Complete)).then_some(progress.received)
    }
}

impl std::fmt::Debug for Stored {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Stored")
            .field("position", &self.position)
            .field("progress", &*self.shared.progress())
            .finish_non_exhaustive()
    }
}

impl Shared {
    fn progress(&self) -> MutexGuard<'_, Progress> {
        self.progress.lock()
    }

    fn change(&self, change: impl FnOnce(&mut Progress)) {
        change(&mut self.progress());
        self.changed.notify_all();
    }

    fn until(&self, ready: impl Fn(&Progress) -> bool) -> MutexGuard<'_, Progress> {
        self.changed
            .wait_while(self.progress(), |progress| !ready(progress))
    }
}

fn disk(error: io::Error) -> Error {
    Error::Disk(error.to_string())
}
