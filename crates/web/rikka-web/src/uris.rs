use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_encode, utf8_percent_encode};

const UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

const PATH: &AsciiSet = &UNRESERVED.remove(b'/');

pub(crate) fn html(document: &str) -> String {
    format!(
        "data:text/html;charset=utf-8,{}",
        utf8_percent_encode(document, UNRESERVED)
    )
}

pub(crate) fn file(path: &Path) -> String {
    format!(
        "file://{}",
        percent_encode(path.as_os_str().as_bytes(), PATH)
    )
}
