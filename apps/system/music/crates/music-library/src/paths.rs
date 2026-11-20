use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

pub(crate) fn bytes(path: &Path) -> &[u8] {
    path.as_os_str().as_bytes()
}

pub(crate) fn from_bytes(bytes: Vec<u8>) -> PathBuf {
    PathBuf::from(OsString::from_vec(bytes))
}

pub(crate) fn beneath(folder: &Path) -> (Vec<u8>, Vec<u8>) {
    let base = bytes(folder).strip_suffix(b"/").unwrap_or(bytes(folder));
    let mut low = base.to_vec();
    low.push(b'/');
    let mut high = base.to_vec();
    high.push(b'/' + 1);
    (low, high)
}

pub(crate) fn is_hidden(name: &OsStr) -> bool {
    name.as_bytes().first() == Some(&b'.')
}
