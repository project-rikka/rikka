use std::io::Read;

use proton_rpgp::{CheckUnixTime, DataEncoding, Signer};

use crate::armor::armored_text;
use crate::error::{Error, Result};
use crate::io_capture::IoCapture;
use crate::key_ring::KeyRing;
use crate::private_key::PrivateKey;
use crate::signature_context::SignatureContext;

#[must_use]
#[derive(Debug)]
pub struct Signing {
    keys: Vec<PrivateKey>,
    context: Option<SignatureContext>,
    utf8: bool,
    time: u64,
}

impl Signing {
    pub fn with_key(mut self, key: PrivateKey) -> Self {
        self.keys.push(key);
        self
    }

    pub fn with_keys(mut self, keys: impl IntoIterator<Item = PrivateKey>) -> Self {
        self.keys.extend(keys);
        self
    }

    pub fn with_ring(mut self, ring: &KeyRing) -> Self {
        self.keys.extend(ring.private_keys().iter().cloned());
        self
    }

    pub fn with_context(mut self, context: SignatureContext) -> Self {
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

    pub fn detached(self, data: impl AsRef<[u8]>) -> Result<Vec<u8>> {
        Ok(self
            .signer()
            .sign_detached(data.as_ref(), DataEncoding::Unarmored)?)
    }

    pub fn armored_detached(self, data: impl AsRef<[u8]>) -> Result<String> {
        let signature = self
            .signer()
            .sign_detached(data.as_ref(), DataEncoding::Armored)?;
        armored_text(signature)
    }

    pub fn inline(self, data: impl AsRef<[u8]>) -> Result<Vec<u8>> {
        Ok(self.signer().sign(data.as_ref(), DataEncoding::Unarmored)?)
    }

    pub fn armored_inline(self, data: impl AsRef<[u8]>) -> Result<String> {
        let message = self.signer().sign(data.as_ref(), DataEncoding::Armored)?;
        armored_text(message)
    }

    pub fn cleartext(self, data: impl AsRef<[u8]>) -> Result<String> {
        let message = self.signer().sign_cleartext(data.as_ref())?;
        armored_text(message)
    }

    pub fn detached_stream(self, source: impl Read) -> Result<Vec<u8>> {
        self.detached_stream_as(source, DataEncoding::Unarmored)
    }

    pub fn armored_detached_stream(self, source: impl Read) -> Result<String> {
        armored_text(self.detached_stream_as(source, DataEncoding::Armored)?)
    }

    pub(crate) fn new(time: u64) -> Self {
        Signing {
            keys: Vec::new(),
            context: None,
            utf8: false,
            time,
        }
    }

    fn signer(&self) -> Signer<'_> {
        let mut signer = Signer::default()
            .with_signing_keys(self.keys.iter().map(PrivateKey::as_inner))
            .at_date(CheckUnixTime::new(self.time));

        if let Some(context) = &self.context {
            signer = signer.with_signature_context(proton_rpgp::SignatureContext::from(context));
        }
        if self.utf8 {
            signer = signer.as_utf8();
        }
        signer
    }

    fn detached_stream_as(&self, source: impl Read, encoding: DataEncoding) -> Result<Vec<u8>> {
        let mut source = IoCapture::new(source);
        let signed = self.hash_detached(&mut source, encoding);
        if let Some(error) = source.take_error() {
            return Err(error);
        }
        signed
    }

    fn hash_detached(&self, source: &mut impl Read, encoding: DataEncoding) -> Result<Vec<u8>> {
        let mut generator = self.signer().sign_detached_stream(source, encoding)?;
        generator
            .discard_all_data()
            .map_err(|error| Error::SigningFailed(Box::new(error)))?;
        generator
            .finalize()
            .map_err(|error| Error::SigningFailed(Box::new(error)))
    }
}
