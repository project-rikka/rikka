use std::path::{Component, Path, PathBuf, Prefix};

use percent_encoding::{percent_decode_str, percent_encode};

use super::{ESCAPED, is_local, split_file_uri};

#[must_use]
pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let (authority, path) = split_file_uri(uri)?;
    let decoded = String::from_utf8(percent_decode_str(path).collect()).ok()?;
    if !is_local(authority) {
        return Some(from_slashed(&format!("//{authority}/{decoded}")));
    }
    match decoded.starts_with('/') || is_drive(&decoded) {
        true => Some(from_slashed(&format!("/{decoded}"))),
        false => None,
    }
}

#[must_use]
pub fn path_to_uri(path: &Path) -> String {
    let encoded = encode_path(path);
    match encoded.starts_with("//") {
        true => format!("file:{encoded}"),
        false => format!("file://{encoded}"),
    }
}

#[must_use]
pub fn encode_path(path: &Path) -> String {
    percent_encode(slashed(path).as_bytes(), ESCAPED).to_string()
}

#[must_use]
pub fn decode_path(encoded: &str) -> PathBuf {
    from_slashed(&percent_decode_str(encoded).decode_utf8_lossy())
}

fn slashed(path: &Path) -> String {
    let mut prefix = String::new();
    let mut rooted = false;
    let mut segments: Vec<String> = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(found) => match found.kind() {
                Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                    prefix = format!("/{}:", char::from(letter).to_ascii_uppercase());
                }
                Prefix::UNC(server, share) | Prefix::VerbatimUNC(server, share) => {
                    prefix = format!("//{}/{}", server.to_string_lossy(), share.to_string_lossy());
                }
                Prefix::Verbatim(_) | Prefix::DeviceNS(_) => {
                    prefix = found.as_os_str().to_string_lossy().replace('\\', "/");
                }
            },
            Component::RootDir => rooted = true,
            Component::CurDir => segments.push(".".to_owned()),
            Component::ParentDir => segments.push("..".to_owned()),
            Component::Normal(name) => segments.push(name.to_string_lossy().into_owned()),
        }
    }
    let root = if rooted { "/" } else { "" };
    format!("{prefix}{root}{}", segments.join("/"))
}

fn from_slashed(slashed: &str) -> PathBuf {
    if let Some(shared) = slashed.strip_prefix("//") {
        return PathBuf::from(format!(r"\\{}", shared.replace('/', "\\")));
    }
    let local = slashed
        .strip_prefix('/')
        .filter(|rest| is_drive(rest))
        .unwrap_or(slashed);
    let mut text = local.replace('/', "\\");
    if is_drive(&text) {
        text.replace_range(1..2, ":");
        if text.len() == 2 {
            text.push('\\');
        }
    }
    PathBuf::from(text)
}

fn is_drive(text: &str) -> bool {
    match text.as_bytes() {
        [letter, b':' | b'|'] => letter.is_ascii_alphabetic(),
        [letter, b':' | b'|', b'/' | b'\\', ..] => letter.is_ascii_alphabetic(),
        _ => false,
    }
}
