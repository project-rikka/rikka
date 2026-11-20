use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MAX_SYMLINK_DEPTH: usize = 40;

pub fn resolve_symlinks(path: &Path) -> io::Result<PathBuf> {
    let mut at = path.to_path_buf();
    for _ in 0..MAX_SYMLINK_DEPTH {
        match fs::symlink_metadata(&at) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let link_target = fs::read_link(&at)?;
                at = match at.parent() {
                    Some(parent) => parent.join(link_target),
                    None => link_target,
                };
            }
            Ok(_) => return Ok(at),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(at),
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("too many levels of symbolic links: {}", path.display()),
    ))
}
