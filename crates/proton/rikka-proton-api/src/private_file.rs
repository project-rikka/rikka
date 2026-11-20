use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use rikka_fs::Replace;

const FOLDER_MODE: u32 = 0o700;

pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(FOLDER_MODE))
}

pub fn write(path: &Path, contents: &[u8]) -> io::Result<()> {
    Replace::new(path).private().write(contents)
}
