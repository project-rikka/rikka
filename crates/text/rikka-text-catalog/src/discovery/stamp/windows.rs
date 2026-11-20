use std::fs::{File, Metadata, OpenOptions};
use std::io;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
};

use crate::discovery::stamp::FileStamp;

const TICKS_PER_SECOND: u64 = 10_000_000;
const NANOS_PER_TICK: u64 = 100;
const UNIX_EPOCH_TICKS: u64 = 116_444_736_000_000_000;

impl FileStamp {
    pub(crate) fn of_file(file: &File) -> io::Result<Self> {
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &raw mut info) }?;
        let written = u64::from(info.ftLastWriteTime.dwHighDateTime) << 32
            | u64::from(info.ftLastWriteTime.dwLowDateTime);
        let since_epoch = i128::from(written) - i128::from(UNIX_EPOCH_TICKS);
        let ticks_per_second = i128::from(TICKS_PER_SECOND);
        Ok(Self {
            size: u64::from(info.nFileSizeHigh) << 32 | u64::from(info.nFileSizeLow),
            modified: since_epoch.div_euclid(ticks_per_second) as i64,
            modified_nanos: (since_epoch.rem_euclid(ticks_per_second) as u64 * NANOS_PER_TICK)
                as u32,
            inode: u64::from(info.nFileIndexHigh) << 32 | u64::from(info.nFileIndexLow),
            device: u64::from(info.dwVolumeSerialNumber),
        })
    }

    pub(crate) fn of_path(path: &Path, _: &Metadata) -> io::Result<Self> {
        let file = OpenOptions::new()
            .access_mode(0)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0)
            .open(path)?;
        Self::of_file(&file)
    }
}
