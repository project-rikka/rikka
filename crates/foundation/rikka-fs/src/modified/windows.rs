use std::fs::OpenOptions;
use std::io;
use std::os::windows::fs::OpenOptionsExt as _;
use std::path::Path;
use std::time::SystemTime;

use windows::Win32::Storage::FileSystem::{FILE_FLAG_BACKUP_SEMANTICS, FILE_WRITE_ATTRIBUTES};

pub fn set_modified(path: &Path, modified: SystemTime) -> io::Result<()> {
    OpenOptions::new()
        .access_mode(FILE_WRITE_ATTRIBUTES.0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(path)?
        .set_modified(modified)
}
