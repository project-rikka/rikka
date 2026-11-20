use std::fs;
use std::path::{Path, PathBuf};

pub fn file_name(uri: &str) -> &str {
    uri.rsplit(['/', '\\']).next().unwrap_or(uri)
}
pub(crate) fn key(uri: &str) -> String {
    relative_parts(uri)
        .unwrap_or_default()
        .join("/")
        .to_lowercase()
}

pub fn relative_parts(uri: &str) -> Option<Vec<String>> {
    let mut parts: Vec<String> = Vec::new();
    for part in uri.replace('\\', "/").split('/') {
        match part {
            "" | "." => {}
            ".." => match parts.pop() {
                Some(_) => {}
                None => return None,
            },
            drive if parts.is_empty() && is_drive(drive) => {}
            name => parts.push(name.to_owned()),
        }
    }
    Some(parts)
}

fn is_drive(part: &str) -> bool {
    let bytes = part.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub(crate) fn sibling(directory: &Path, name: &str) -> Option<PathBuf> {
    let lowercase_name = name.to_lowercase();
    fs::read_dir(directory).ok()?.flatten().find_map(|entry| {
        let file_name = entry.file_name();
        (file_name.to_string_lossy().to_lowercase() == lowercase_name).then(|| entry.path())
    })
}

pub(crate) fn contained(root: &Path, path: &Path) -> Option<PathBuf> {
    let root = root.canonicalize().ok()?;
    let path = path.canonicalize().ok()?;
    path.starts_with(&root).then_some(path)
}
