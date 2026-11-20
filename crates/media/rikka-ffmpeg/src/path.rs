use std::ffi::CString;
use std::path::Path;

use crate::error::{Error, Result};

#[cfg(unix)]
fn path_bytes(path: &Path) -> Result<&[u8]> {
    use std::os::unix::ffi::OsStrExt;
    Ok(path.as_os_str().as_bytes())
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Result<&[u8]> {
    path.to_str()
        .map(str::as_bytes)
        .ok_or_else(|| Error::Unsupported("path is not valid Unicode".into()))
}

pub(crate) fn c_path(path: &Path) -> Result<CString> {
    CString::new(path_bytes(path)?).map_err(|_| Error::Unsupported("path contains NUL".into()))
}
