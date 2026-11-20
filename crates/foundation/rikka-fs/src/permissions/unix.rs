use std::fs::{self, Permissions};
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use crate::replace::{Mode, PERMISSION_BITS};

pub(crate) fn apply(staging: &Path, mode: Mode, target: &Path) -> io::Result<()> {
    fs::set_permissions(staging, Permissions::from_mode(resolve(mode, target)?))
}

fn resolve(mode: Mode, target: &Path) -> io::Result<u32> {
    let default = match mode {
        Mode::Fixed(mode) => return Ok(mode),
        Mode::Preserve { default } => default,
    };
    match fs::metadata(target) {
        Ok(metadata) => Ok(metadata.permissions().mode() & PERMISSION_BITS),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(default),
        Err(error) => Err(error),
    }
}
