use std::fmt;

use crate::tables;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnicodeVersion {
    pub major: u8,
    pub minor: u8,
    pub update: u8,
}

pub const UNICODE_VERSION: UnicodeVersion = {
    let (major, minor, update) = tables::UNICODE_VERSION;
    UnicodeVersion {
        major,
        minor,
        update,
    }
};

impl fmt::Display for UnicodeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.update)
    }
}
