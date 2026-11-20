use std::fmt;

use crate::error::{Error, Result};
use crate::signature_status::SignatureStatus;

#[derive(Clone)]
pub struct Plaintext {
    data: Vec<u8>,
    signature: SignatureStatus,
}

impl Plaintext {
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[must_use]
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    #[must_use]
    pub fn signature(&self) -> &SignatureStatus {
        &self.signature
    }

    pub fn into_verified(self) -> Result<Vec<u8>> {
        if self.signature.is_valid() {
            Ok(self.data)
        } else {
            Err(Error::SignatureInvalid(self.signature))
        }
    }

    pub(crate) fn new(data: Vec<u8>, signature: SignatureStatus) -> Self {
        Plaintext { data, signature }
    }
}

impl fmt::Debug for Plaintext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Plaintext")
            .field("bytes", &self.data.len())
            .field("signature", &self.signature)
            .finish()
    }
}
