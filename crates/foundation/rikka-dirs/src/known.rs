use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt as _;
use std::path::PathBuf;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::core::GUID;

pub(crate) fn path(folder: &GUID) -> Option<PathBuf> {
    let found = unsafe { SHGetKnownFolderPath(folder, KF_FLAG_DEFAULT, None) }.ok()?;
    let path = PathBuf::from(OsString::from_wide(unsafe { found.as_wide() }));
    unsafe { CoTaskMemFree(Some(found.0.cast_const().cast())) };
    path.is_absolute().then_some(path)
}
