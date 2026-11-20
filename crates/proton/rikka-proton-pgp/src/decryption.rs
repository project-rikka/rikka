use std::borrow::Cow;
use std::fmt;
use std::io::{self, Read, Write};

use proton_rpgp::{
    CheckUnixTime, DataEncoding, Decryptor, ExternalDetachedSignature, VerificationResult,
};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::io_capture::IoCapture;
use crate::key_ring::KeyRing;
use crate::plaintext::Plaintext;
use crate::private_key::PrivateKey;
use crate::public_key::PublicKey;
use crate::session_key::SessionKey;
use crate::signature_context::VerificationContext;
use crate::signature_status::SignatureStatus;

#[must_use]
pub struct Decryption {
    keys: Vec<PrivateKey>,
    passwords: Vec<Zeroizing<Vec<u8>>>,
    session_key: Option<SessionKey>,
    verifiers: Vec<PublicKey>,
    detached: Option<Detached>,
    context: Option<VerificationContext>,
    utf8: bool,
    time: u64,
    forwarding: bool,
}

enum Detached {
    Plain(Vec<u8>),
    Encrypted(Vec<u8>),
}

impl Decryption {
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

    pub fn with_password(mut self, password: impl AsRef<[u8]>) -> Self {
        self.passwords
            .push(Zeroizing::new(password.as_ref().to_vec()));
        self
    }

    pub fn with_session_key(mut self, session_key: SessionKey) -> Self {
        self.session_key = Some(session_key);
        self
    }

    pub fn verified_by(mut self, key: PublicKey) -> Self {
        self.verifiers.push(key);
        self
    }

    pub fn verified_by_keys(mut self, keys: impl IntoIterator<Item = PublicKey>) -> Self {
        self.verifiers.extend(keys);
        self
    }

    pub fn verified_by_ring(mut self, ring: &KeyRing) -> Self {
        self.verifiers.extend(ring.all_public_keys());
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

    pub fn with_detached_signature(mut self, signature: impl AsRef<[u8]>) -> Self {
        self.detached = Some(Detached::Plain(signature.as_ref().to_vec()));
        self
    }

    pub fn with_encrypted_detached_signature(mut self, signature: impl AsRef<[u8]>) -> Self {
        self.detached = Some(Detached::Encrypted(signature.as_ref().to_vec()));
        self
    }

    pub fn allow_forwarding(mut self) -> Self {
        self.forwarding = true;
        self
    }

    pub fn message(self, data: impl AsRef<[u8]>) -> Result<Plaintext> {
        let decrypted = self
            .decryptor()
            .decrypt(data.as_ref(), DataEncoding::Auto)?;
        let status = self.status(decrypted.verification_result);
        Ok(Plaintext::new(decrypted.data, status))
    }

    pub fn stream(self, source: impl Read + Send, sink: impl Write) -> Result<SignatureStatus> {
        let mut source = IoCapture::new(source);
        let mut sink = IoCapture::new(sink);
        let verified = self.pump(&mut source, &mut sink);
        if let Some(error) = source.take_error().or(sink.take_error()) {
            return Err(error);
        }
        Ok(self.status(verified?))
    }

    pub fn session_key(self, key_packets: impl AsRef<[u8]>) -> Result<SessionKey> {
        let session_key = self.decryptor().decrypt_session_key(key_packets.as_ref())?;
        Ok(SessionKey::from_inner(session_key))
    }

    pub(crate) fn new(time: u64) -> Self {
        Decryption {
            keys: Vec::new(),
            passwords: Vec::new(),
            session_key: None,
            verifiers: Vec::new(),
            detached: None,
            context: None,
            utf8: false,
            time,
            forwarding: false,
        }
    }

    fn pump(
        &self,
        source: &mut (impl Read + Send),
        sink: &mut impl Write,
    ) -> Result<VerificationResult> {
        let mut reader = self
            .decryptor()
            .decrypt_stream(source, DataEncoding::Auto)?;
        io::copy(&mut reader, sink).map_err(|error| Error::DecryptionFailed(Box::new(error)))?;
        Ok(reader.verification_result())
    }

    fn decryptor(&self) -> Decryptor<'_> {
        let mut decryptor = Decryptor::default()
            .with_decryption_keys(self.keys.iter().map(PrivateKey::as_inner))
            .with_verification_keys(self.verifiers.iter().map(PublicKey::as_inner))
            .with_passphrases(self.passwords.iter().map(|password| password.as_slice()))
            .at_date(CheckUnixTime::new(self.time))
            .allow_forwarding_decryption(self.forwarding);

        if let Some(session_key) = &self.session_key {
            decryptor = decryptor.with_session_key(session_key.as_inner());
        }
        if let Some(context) = &self.context {
            decryptor = decryptor
                .with_verification_context(proton_rpgp::VerificationContext::from(context));
        }
        if self.utf8 {
            decryptor = decryptor.output_utf8();
        }
        if let Some(detached) = &self.detached {
            decryptor = decryptor.with_external_detached_signature(match detached {
                Detached::Plain(signature) => ExternalDetachedSignature::new_unencrypted(
                    Cow::Borrowed(signature.as_slice()),
                    DataEncoding::Auto,
                ),
                Detached::Encrypted(signature) => ExternalDetachedSignature::new_encrypted(
                    Cow::Borrowed(signature.as_slice()),
                    DataEncoding::Auto,
                ),
            });
        }
        decryptor
    }

    fn status(&self, result: VerificationResult) -> SignatureStatus {
        SignatureStatus::from_verification(result, &self.verifiers, self.time)
    }
}

impl fmt::Debug for Decryption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Decryption")
            .field("keys", &self.keys.len())
            .field("passwords", &self.passwords.len())
            .field("session_key", &self.session_key.is_some())
            .field("verifiers", &self.verifiers.len())
            .field("detached_signature", &self.detached.is_some())
            .field("context", &self.context)
            .field("time", &self.time)
            .finish_non_exhaustive()
    }
}
