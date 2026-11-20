use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_LINKED_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn folder_of(document: &Path) -> Option<PathBuf> {
    let folder = match document.parent() {
        Some(folder) if !folder.as_os_str().is_empty() => folder,
        _ => Path::new("."),
    };
    folder.canonicalize().ok()
}

pub(crate) fn linked_bytes(folder: &Path, href: &str) -> Option<Vec<u8>> {
    let link = Path::new(href);
    if link.is_absolute() {
        return None;
    }
    let target = folder.join(link).canonicalize().ok()?;
    if !target.starts_with(folder) {
        return None;
    }
    let file = File::open(&target).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_LINKED_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= MAX_LINKED_BYTES).then_some(bytes)
}
