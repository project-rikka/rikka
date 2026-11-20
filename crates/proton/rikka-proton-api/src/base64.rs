use ::base64::prelude::{BASE64_STANDARD, Engine as _};

pub use ::base64::DecodeError;

#[must_use]
pub fn encode(bytes: impl AsRef<[u8]>) -> String {
    BASE64_STANDARD.encode(bytes)
}

pub fn decode(text: impl AsRef<[u8]>) -> Result<Vec<u8>, DecodeError> {
    BASE64_STANDARD.decode(text)
}
