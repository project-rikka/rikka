use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(pub [u8; 4]);

impl Key {
    pub const fn new(code: &[u8; 4]) -> Self {
        Self(*code)
    }

    pub const fn bytes(self) -> [u8; 4] {
        self.0
    }

    pub fn as_str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }
}

impl From<&[u8; 4]> for Key {
    fn from(code: &[u8; 4]) -> Self {
        Self(*code)
    }
}

impl PartialEq<&[u8; 4]> for Key {
    fn eq(&self, other: &&[u8; 4]) -> bool {
        &self.0 == *other
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            if byte.is_ascii_graphic() || byte == b' ' {
                write!(f, "{}", byte as char)?;
            } else {
                write!(f, "\\x{byte:02x}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Key(\"{self}\")")
    }
}
