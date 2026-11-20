use std::fs::{File, Metadata};
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::discovery::stamp::FileStamp;

impl FileStamp {
    pub(crate) fn of_file(file: &File) -> io::Result<Self> {
        Ok(Self::of_metadata(&file.metadata()?))
    }

    pub(crate) fn of_path(_: &Path, metadata: &Metadata) -> io::Result<Self> {
        Ok(Self::of_metadata(metadata))
    }

    fn of_metadata(metadata: &Metadata) -> Self {
        Self {
            size: metadata.size(),
            modified: metadata.mtime(),
            modified_nanos: u32::try_from(metadata.mtime_nsec()).unwrap_or(0),
            inode: metadata.ino(),
            device: metadata.dev(),
        }
    }
}
