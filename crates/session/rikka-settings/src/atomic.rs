use std::fs::{self, File};
use std::io;
use std::path::Path;

pub(crate) fn read(path: &Path) -> io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => flush(path.parent().unwrap_or_else(|| Path::new("."))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn flush(directory: &Path) -> io::Result<()> {
    let handle = File::open(directory)?;
    match handle.sync_all() {
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
        outcome => outcome,
    }
}
