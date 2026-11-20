use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use super::Refusal;

pub(crate) fn open(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
}

pub(crate) fn create(path: &Path) -> Result<File, Refusal> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| match error.raw_os_error() {
            Some(libc::ELOOP) => Refusal::Link,
            _ => Refusal::Io(error),
        })
}
