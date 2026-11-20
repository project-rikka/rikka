#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FileStamp {
    pub(crate) size: u64,
    pub(crate) modified: i64,
    pub(crate) modified_nanos: u32,
    pub(crate) inode: u64,
    pub(crate) device: u64,
}

impl FileStamp {
    pub(crate) fn same_content(&self, other: &Self) -> bool {
        self.size == other.size
            && self.modified == other.modified
            && self.modified_nanos == other.modified_nanos
            && self.inode == other.inode
    }

    pub(crate) fn identity(&self) -> (u64, u64) {
        (self.device, self.inode)
    }
}
