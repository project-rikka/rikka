use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use rikka_file_format::DecodeError;

pub(crate) fn encode(path: &Path) -> Option<&[u8]> {
    Some(path.as_os_str().as_bytes())
}

pub(crate) fn decode(bytes: &[u8]) -> Result<PathBuf, DecodeError> {
    Ok(PathBuf::from(OsStr::from_bytes(bytes)))
}
