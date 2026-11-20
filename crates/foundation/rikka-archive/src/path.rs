use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub(crate) fn normalize(raw: &str) -> String {
    let mut name = raw.replace('\\', "/");

    if let Some(rest) = strip_drive_prefix(&name) {
        name = rest.to_owned();
    }

    let trimmed = name.trim_start_matches('/');
    let mut parts = Vec::new();
    for part in trimmed.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        parts.push(part);
    }

    let mut normalized = parts.join("/");
    if normalized.is_empty() {
        return normalized;
    }
    if raw.ends_with('/') || raw.ends_with('\\') {
        normalized.push('/');
    }
    normalized
}

pub(crate) fn relative(name: &str) -> Result<PathBuf> {
    let unsafe_path = || Error::UnsafeEntryPath(name.to_owned());

    if strip_drive_prefix(name).is_some() || name.starts_with('/') || name.starts_with('\\') {
        return Err(unsafe_path());
    }

    let mut path = PathBuf::new();
    for part in name.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(unsafe_path());
        }
        path.push(part);
    }

    if path.as_os_str().is_empty() {
        return Err(unsafe_path());
    }
    Ok(path)
}

pub(crate) fn validate(raw: &str) -> Result<String> {
    let normalized = normalize(raw);
    relative(&normalized)?;
    Ok(normalized)
}

pub(crate) fn join(root: &Path, name: &str) -> Result<PathBuf> {
    Ok(root.join(relative(name)?))
}

pub(crate) fn link_stays_within(root: &Path, link_path: &Path, target: &str) -> bool {
    if strip_drive_prefix(target).is_some() || target.starts_with('/') || target.starts_with('\\') {
        return false;
    }

    let Some(parent) = link_path.parent() else {
        return false;
    };

    let Ok(mut depth) = parent
        .strip_prefix(root)
        .map(|rest| rest.components().count())
    else {
        return false;
    };

    let mut descended = false;
    for part in target.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                if descended || depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            _ => descended = true,
        }
    }
    true
}

pub(crate) fn crosses_link(root: &Path, path: &Path) -> io::Result<bool> {
    let Ok(below) = path.strip_prefix(root) else {
        return Ok(true);
    };
    let mut walked = root.to_path_buf();
    for part in below.components() {
        walked.push(part);
        match walked.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

fn strip_drive_prefix(name: &str) -> Option<&str> {
    let bytes = name.as_bytes();
    let is_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    is_drive.then(|| &name[2..])
}
