use std::fmt;
use std::str::FromStr;

use crate::error::Error;

const CAPACITY: usize = 31;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Language {
    bytes: [u8; CAPACITY],
    len: u8,
}

impl Language {
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)])
            .expect("language tags are validated as ASCII")
    }
}

impl FromStr for Language {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Error> {
        let source = text.as_bytes();
        let valid = !source.is_empty()
            && source.len() <= CAPACITY
            && source
                .iter()
                .all(|&byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        if !valid {
            return Err(Error::InvalidLanguage(text.to_owned()));
        }
        let mut bytes = [0u8; CAPACITY];
        for (target, &byte) in bytes.iter_mut().zip(source) {
            *target = if byte == b'_' {
                b'-'
            } else {
                byte.to_ascii_lowercase()
            };
        }
        Ok(Self {
            bytes,
            len: source.len() as u8,
        })
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Language({})", self.as_str())
    }
}
