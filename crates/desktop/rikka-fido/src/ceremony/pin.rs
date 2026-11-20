use std::fmt;

use zeroize::Zeroizing;

const MAX_BYTES: usize = 63;
const PADDED: usize = 64;
const FLOOR: usize = 4;

#[derive(Clone)]
pub struct Pin(Zeroizing<String>);

impl Pin {
    #[must_use]
    pub fn new(pin: impl Into<String>) -> Pin {
        Pin(Zeroizing::new(pin.into()))
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub(crate) fn is_valid_length(&self, min_length: usize) -> bool {
        self.0.chars().count() >= min_length.max(FLOOR) && self.0.len() <= MAX_BYTES
    }

    pub(crate) fn padded(&self) -> Zeroizing<[u8; PADDED]> {
        let mut padded = Zeroizing::new([0u8; PADDED]);
        let length = self.0.len().min(MAX_BYTES);
        padded[..length].copy_from_slice(&self.0.as_bytes()[..length]);
        padded
    }
}

impl From<String> for Pin {
    fn from(pin: String) -> Pin {
        Pin::new(pin)
    }
}

impl From<&str> for Pin {
    fn from(pin: &str) -> Pin {
        Pin::new(pin)
    }
}

impl fmt::Debug for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Pin(<redacted>)")
    }
}
