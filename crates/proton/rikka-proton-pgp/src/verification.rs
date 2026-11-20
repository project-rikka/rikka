use std::io::Read;

use proton_rpgp::{CheckUnixTime, DataEncoding, VerificationError, VerificationResult, Verifier};

use crate::error::Result;
use crate::io_capture::IoCapture;
use crate::key_ring::KeyRing;
use crate::plaintext::Plaintext;
use crate::public_key::PublicKey;
use crate::signature_context::VerificationContext;
use crate::signature_status::SignatureStatus;

#[must_use]
#[derive(Debug)]
pub struct Verification {
    keys: Vec<PublicKey>,
    context: Option<VerificationContext>,
    utf8: bool,
    time: u64,
}

impl Verification {
    pub fn with_key(mut self, key: PublicKey) -> Self {
        self.keys.push(key);
        self
    }

    pub fn with_keys(mut self, keys: impl IntoIterator<Item = PublicKey>) -> Self {
        self.keys.extend(keys);
        self
    }

    pub fn with_ring(mut self, ring: &KeyRing) -> Self {
        self.keys.extend(ring.all_public_keys());
        self
    }

    pub fn with_context(mut self, context: VerificationContext) -> Self {
        self.context = Some(context);
        self
    }

    pub fn as_utf8(mut self) -> Self {
        self.utf8 = true;
        self
    }

    pub fn with_time(mut self, unix_seconds: u64) -> Self {
        self.time = unix_seconds;
        self
    }

    #[must_use]
    pub fn detached(self, data: impl AsRef<[u8]>, signature: impl AsRef<[u8]>) -> SignatureStatus {
        let result =
            self.verifier()
                .verify_detached(data.as_ref(), signature.as_ref(), DataEncoding::Auto);
        self.status(result)
    }

    pub fn detached_stream(
        self,
        data: impl Read,
        signature: impl AsRef<[u8]>,
    ) -> Result<SignatureStatus> {
        let mut data = IoCapture::new(data);
        let result = self.hash_detached(&mut data, signature.as_ref());
        if let Some(error) = data.take_error() {
            return Err(error);
        }
        Ok(self.status(result))
    }

    pub fn inline(self, message: impl AsRef<[u8]>) -> Result<Plaintext> {
        let verified = self
            .verifier()
            .verify(message.as_ref(), DataEncoding::Auto)?;
        let status = self.status(verified.verification_result);
        Ok(Plaintext::new(verified.data, status))
    }

    pub fn cleartext(self, message: impl AsRef<[u8]>) -> Result<Plaintext> {
        let verified = self.verifier().verify_cleartext(message.as_ref())?;
        let status = self.status(verified.verification_result);
        Ok(Plaintext::new(verified.data, status))
    }

    pub(crate) fn new(time: u64) -> Self {
        Verification {
            keys: Vec::new(),
            context: None,
            utf8: false,
            time,
        }
    }

    fn verifier(&self) -> Verifier<'_> {
        let mut verifier = Verifier::default()
            .with_verification_keys(self.keys.iter().map(PublicKey::as_inner))
            .at_date(CheckUnixTime::new(self.time));

        if let Some(context) = &self.context {
            verifier =
                verifier.with_verification_context(proton_rpgp::VerificationContext::from(context));
        }
        if self.utf8 {
            verifier = verifier.output_utf8();
        }
        verifier
    }

    fn hash_detached(&self, data: &mut impl Read, signature: &[u8]) -> VerificationResult {
        let mut reader = self
            .verifier()
            .verify_detached_stream(data, signature, DataEncoding::Auto)
            .map_err(|error| VerificationError::RuntimeError(error.to_string()))?;
        reader
            .discard_all_data()
            .map_err(|error| VerificationError::RuntimeError(error.to_string()))?;
        reader.verification_result()
    }

    fn status(&self, result: VerificationResult) -> SignatureStatus {
        SignatureStatus::from_verification(result, &self.keys, self.time)
    }
}
