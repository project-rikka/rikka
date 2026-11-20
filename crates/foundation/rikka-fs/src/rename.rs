use std::io;
use std::path::Path;

#[cfg(unix)]
pub fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    use rustix::fs::{CWD, RenameFlags, renameat_with};
    use rustix::io::Errno;

    match renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(()),
        Err(Errno::INVAL | Errno::NOSYS | Errno::NOTSUP) => {
            if std::fs::symlink_metadata(to).is_ok() {
                return Err(io::ErrorKind::AlreadyExists.into());
            }
            std::fs::rename(from, to)
        }
        Err(errno) => Err(errno.into()),
    }
}

#[cfg(unix)]
pub(crate) fn rename_over(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::rename(from, to)
}

#[cfg(windows)]
pub fn rename_new(from: &Path, to: &Path) -> io::Result<()> {
    move_file(
        from,
        to,
        windows::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH,
    )
}

#[cfg(windows)]
pub(crate) fn rename_over(from: &Path, to: &Path) -> io::Result<()> {
    use windows::Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};

    move_file(from, to, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)
}

#[cfg(windows)]
fn move_file(
    from: &Path,
    to: &Path,
    flags: windows::Win32::Storage::FileSystem::MOVE_FILE_FLAGS,
) -> io::Result<()> {
    use windows::core::HSTRING;

    unsafe {
        windows::Win32::Storage::FileSystem::MoveFileExW(
            &HSTRING::from(from),
            &HSTRING::from(to),
            flags,
        )
    }
    .map_err(io::Error::from)
}
