use std::path::{Path, PathBuf};

use rikka_file_format::DecodeError;

pub(crate) fn encode(path: &Path) -> Option<&[u8]> {
    path.to_str().map(str::as_bytes)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<PathBuf, DecodeError> {
    std::str::from_utf8(bytes)
        .map(PathBuf::from)
        .map_err(|_| DecodeError::Malformed("font path is not utf-8"))
}
