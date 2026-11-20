use std::fmt;

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId([u8; FileId::LEN]);

impl FileId {
    pub const LEN: usize = 20;

    #[must_use]
    pub fn new(bytes: [u8; FileId::LEN]) -> FileId {
        FileId(bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<FileId> {
        if bytes.is_empty() || bytes.len() > FileId::LEN {
            return Err(Error::Id(rikka_types::hex::encode(bytes)));
        }
        let mut padded = [0u8; FileId::LEN];
        padded[..bytes.len()].copy_from_slice(bytes);
        Ok(FileId(padded))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; FileId::LEN] {
        &self.0
    }

    #[must_use]
    pub fn to_base16(&self) -> String {
        rikka_types::hex::encode(self.0)
    }
}

impl fmt::Display for FileId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_base16())
    }
}
