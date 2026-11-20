use crate::entry::EntryKind;

pub(crate) const S_IFMT: u32 = 0o170_000;
pub(crate) const S_IFLNK: u32 = 0o120_000;
pub(crate) const S_IFREG: u32 = 0o100_000;
pub(crate) const S_IFDIR: u32 = 0o040_000;
pub(crate) const PERMISSION_BITS: u32 = 0o7777;

const UNIX_EXTENSION: u32 = 0x8000;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;

pub(crate) fn kind_of(mode: Option<u32>, is_directory: bool) -> EntryKind {
    if mode.is_some_and(|mode| mode & S_IFMT == S_IFLNK) {
        EntryKind::Symlink
    } else if is_directory {
        EntryKind::Directory
    } else {
        EntryKind::File
    }
}

pub(crate) fn from_windows_attributes(attributes: u32) -> Option<u32> {
    (attributes & UNIX_EXTENSION != 0).then_some(attributes >> 16)
}

pub(crate) fn to_windows_attributes(mode: u32, file_type: u32, directory: bool) -> u32 {
    let mut attributes = ((mode & PERMISSION_BITS) | file_type) << 16 | UNIX_EXTENSION;
    if directory {
        attributes |= FILE_ATTRIBUTE_DIRECTORY;
    }
    attributes
}
