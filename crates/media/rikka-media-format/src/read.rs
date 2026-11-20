use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::format::Format;
use crate::kind::Kind;

const HEADER: u64 = 8 * 1024;

#[must_use]
pub fn identify(path: impl AsRef<Path>) -> Option<Format> {
    let path = path.as_ref();
    let header = header(path).ok()?;
    Format::identify(path, &header)
}

#[must_use]
pub fn kind(path: impl AsRef<Path>) -> Option<Kind> {
    identify(path).map(Format::kind)
}

fn header(path: &Path) -> std::io::Result<Vec<u8>> {
    let file = File::open(path)?;
    if file.metadata()?.is_dir() {
        return Err(std::io::Error::from(std::io::ErrorKind::IsADirectory));
    }
    let mut header = Vec::new();
    file.take(HEADER).read_to_end(&mut header)?;
    Ok(header)
}
