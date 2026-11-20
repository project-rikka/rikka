use std::ffi::OsString;
use std::path::Path;

use crate::error::{Error, Result};
use crate::shell;

pub(super) fn open(target: &str) -> Result<()> {
    let file = match crate::uri_to_path(target) {
        Some(path) => path.into_os_string(),
        None => OsString::from(target),
    };
    shell::in_apartment(move || shell::execute(&file)).map_err(|source| Error::Open {
        target: target.to_owned(),
        source,
    })
}

pub(super) fn reveal(path: &Path) -> Result<()> {
    let shown = path.to_path_buf();
    shell::in_apartment(move || shell::select(&shown)).map_err(|source| Error::Open {
        target: path.display().to_string(),
        source,
    })
}
