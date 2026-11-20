use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

use crate::error::{Error, Result};
use crate::extract::options::Overwrite;

pub(crate) fn prepare_destination(path: &Path, overwrite: Overwrite) -> Result<bool> {
    let Ok(metadata) = path.symlink_metadata() else {
        return Ok(true);
    };

    match overwrite {
        Overwrite::Skip => Ok(false),
        Overwrite::Fail => Err(Error::AlreadyExists(path.to_path_buf())),
        Overwrite::Replace => {
            if metadata.is_dir() {
                return Ok(true);
            }
            fs::remove_file(path)?;
            Ok(true)
        }
    }
}

pub(crate) fn apply_mode(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(mode & 0o7777))
}

pub(crate) fn apply_modified(path: &Path, modified: SystemTime) -> io::Result<()> {
    rikka_fs::set_modified(path, modified)
}

pub(crate) fn create_symlink(target: &str, path: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, path)
}
