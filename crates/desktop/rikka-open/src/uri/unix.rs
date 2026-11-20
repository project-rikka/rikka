use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use percent_encoding::{percent_decode_str, percent_encode};

use super::{ESCAPED, is_local, split_file_uri};

#[must_use]
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let (authority, path) = split_file_uri(uri)?;
    if !is_local(authority) {
        return None;
    }
    let mut decoded = vec![b'/'];
    decoded.extend(percent_decode_str(path));
    Some(PathBuf::from(OsString::from_vec(decoded)))
}

#[must_use]
pub fn path_to_uri(path: &Path) -> String {
    format!("file://{}", encode_path(path))
}

#[must_use]
pub fn encode_path(path: &Path) -> String {
    percent_encode(path.as_os_str().as_bytes(), ESCAPED).to_string()
}

#[must_use]
pub fn decode_path(encoded: &str) -> PathBuf {
    PathBuf::from(OsString::from_vec(percent_decode_str(encoded).collect()))
}
