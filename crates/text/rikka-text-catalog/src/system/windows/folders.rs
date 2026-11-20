use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Fonts, FOLDERID_LocalAppData, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
};
use windows::core::GUID;

const USER_FONTS: [&str; 3] = ["Microsoft", "Windows", "Fonts"];

pub(crate) fn system_fonts() -> io::Result<PathBuf> {
    known_folder(&FOLDERID_Fonts)
}

pub(crate) fn user_fonts() -> io::Result<PathBuf> {
    Ok(USER_FONTS
        .iter()
        .fold(known_folder(&FOLDERID_LocalAppData)?, |path, part| {
            path.join(part)
        }))
}

fn known_folder(id: &GUID) -> io::Result<PathBuf> {
    let path = unsafe { SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None) }?;
    let wide = OsString::from_wide(unsafe { path.as_wide() });
    unsafe { CoTaskMemFree(Some(path.0.cast_const().cast())) };
    Ok(PathBuf::from(wide))
}
