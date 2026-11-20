use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub(super) const SEPARATORS: [char; 1] = ['/'];

#[must_use]
pub fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}

pub(super) fn runnable_in(directory: &Path, program: &str) -> Option<PathBuf> {
    let path = directory.join(program);
    is_executable(&path).then_some(path)
}
