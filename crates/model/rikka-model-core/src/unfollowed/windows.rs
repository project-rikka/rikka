use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;

use windows::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;

use super::Refusal;

pub(crate) fn open(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
}

pub(crate) fn create(path: &Path) -> Result<File, Refusal> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)
        .map_err(Refusal::Io)?;
    match file
        .metadata()
        .map_err(Refusal::Io)?
        .file_type()
        .is_symlink()
    {
        true => Err(Refusal::Link),
        false => Ok(file),
    }
}
