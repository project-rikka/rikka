#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::path::Path;
use std::ptr::NonNull;
use std::sync::OnceLock;

#[cfg(unix)]
use unix::{map, unmap};
#[cfg(windows)]
use windows::{map, unmap};

use crate::discovery::stamp::FileStamp;

#[derive(Debug)]
pub(crate) struct Mapping {
    start: NonNull<u8>,
    len: usize,
}

unsafe impl Send for Mapping {}
unsafe impl Sync for Mapping {}

impl Mapping {
    pub(crate) fn open(path: &Path) -> io::Result<(Self, FileStamp)> {
        let file = File::open(path)?;
        let stamp = FileStamp::of_file(&file)?;
        let len = usize::try_from(stamp.size)
            .ok()
            .filter(|&len| len > 0)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "font file is empty"))?;
        let start = map(&file, len)?;
        Ok((Self { start, len }, stamp))
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.start.as_ptr(), self.len) }
    }

    fn leak(self) -> &'static [u8] {
        let bytes = unsafe { std::slice::from_raw_parts(self.start.as_ptr(), self.len) };
        std::mem::forget(self);
        bytes
    }
}

impl Drop for Mapping {
    fn drop(&mut self) {
        unsafe { unmap(self.start, self.len) };
    }
}

fn registry() -> MutexGuard<'static, HashMap<FileStamp, &'static [u8]>> {
    static MAPPED: OnceLock<Mutex<HashMap<FileStamp, &'static [u8]>>> = OnceLock::new();
    MAPPED.get_or_init(Mutex::default).lock()
}

pub(crate) fn mapped_file(path: &Path, expected: &FileStamp) -> Option<&'static [u8]> {
    if let Some(&bytes) = registry().get(expected) {
        return Some(bytes);
    }
    let (mapping, stamp) = Mapping::open(path).ok()?;
    if !stamp.same_content(expected) {
        return None;
    }
    Some(
        *registry()
            .entry(*expected)
            .or_insert_with(|| mapping.leak()),
    )
}

pub fn mapped_font(path: &Path) -> io::Result<&'static [u8]> {
    let (mapping, stamp) = Mapping::open(path)?;
    Ok(*registry().entry(stamp).or_insert_with(|| mapping.leak()))
}
