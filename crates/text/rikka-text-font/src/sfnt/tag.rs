use std::fmt;

use crate::parse::array::Scalar;

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Tag(u32);

impl Tag {
    #[inline]
    pub const fn new(bytes: &[u8; 4]) -> Self {
        Self(u32::from_be_bytes(*bytes))
    }

    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        Self(value)
    }

    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.is_empty() || bytes.len() > 4 || !bytes.iter().all(|b| (0x20..=0x7E).contains(b)) {
            return None;
        }
        let mut padded = [b' '; 4];
        padded[..bytes.len()].copy_from_slice(bytes);
        Some(Self::new(&padded))
    }

    #[inline]
    pub const fn to_u32(self) -> u32 {
        self.0
    }

    #[inline]
    pub const fn to_bytes(self) -> [u8; 4] {
        self.0.to_be_bytes()
    }
}

impl Scalar for Tag {
    const SIZE: usize = 4;

    #[inline]
    fn parse(bytes: &[u8]) -> Option<Self> {
        bytes.first_chunk().map(|b| Self(u32::from_be_bytes(*b)))
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.to_bytes() {
            let shown = if (0x20..=0x7E).contains(&byte) {
                byte as char
            } else {
                '?'
            };
            fmt::Write::write_char(f, shown)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tag({self})")
    }
}
