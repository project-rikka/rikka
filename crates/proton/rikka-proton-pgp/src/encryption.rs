use std::fmt;
use std::io::{Read, Write};

use proton_rpgp::{CheckUnixTime, DataEncoding, Encryptor};
use zeroize::Zeroizing;

use crate::armor::armored_text;
use crate::error::{Error, Result};
use crate::io_capture::IoCapture;
use crate::key_ring::KeyRing;
use crate::private_key::PrivateKey;
use crate::public_key::PublicKey;
use crate::session_key::SessionKey;
use crate::signature_context::SignatureContext;
use crate::split_keys::SplitKeys;
use crate::split_message::SplitMessage;

#[must_use]
pub struct Encryption {
    recipients: Vec<PublicKey>,
    passwords: Vec<Zeroizing<Vec<u8>>>,
    signers: Vec<PrivateKey>,
    session_key: Option<SessionKey>,
    context: Option<SignatureContext>,
    utf8: bool,
    time: u64,
}

impl Encryption {
    pub fn to(mut self, key: PublicKey) -> Self {
        self.recipients.push(key);
        self
    }

    pub fn to_keys(mut self, keys: impl IntoIterator<Item = PublicKey>) -> Self {
        self.recipients.extend(keys);
        self
    }

    pub fn to_ring(mut self, ring: &KeyRing) -> Self {
        self.recipients.extend(ring.all_public_keys());
        self
    }

    pub fn with_password(mut self, password: impl AsRef<[u8]>) -> Self {
        self.passwords
            .push(Zeroizing::new(password.as_ref().to_vec()));
        self
    }

    pub fn signed_by(mut self, key: PrivateKey) -> Self {
        self.signers.push(key);
        self
    }

    pub fn signed_by_keys(mut self, keys: impl IntoIterator<Item = PrivateKey>) -> Self {
        self.signers.extend(keys);
        self
    }

    pub fn with_session_key(mut self, session_key: SessionKey) -> Self {
        self.session_key = Some(session_key);
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

    pub fn message(self, data: impl AsRef<[u8]>) -> Result<Vec<u8>> {
        let message = self.encryptor().encrypt(data.as_ref())?;
        Ok(message.encrypted_data)
    }

    pub fn armored_message(self, data: impl AsRef<[u8]>) -> Result<String> {
        let message = self.encryptor().encrypt(data.as_ref())?;
        let armored = message.armor()?;
        armored_text(armored)
    }

    pub fn split(self, data: impl AsRef<[u8]>) -> Result<SplitMessage> {
        let mut data_packet = Vec::new();
        let (key_packets, session_key) = self
            .split_stream(data.as_ref(), &mut data_packet)?
            .into_parts();
        Ok(SplitMessage::new(key_packets, data_packet, session_key))
    }

    pub fn stream(self, source: impl Read, sink: impl Write) -> Result<()> {
        self.stream_as(source, sink, DataEncoding::Unarmored)
    }

    pub fn armored_stream(self, source: impl Read, sink: impl Write) -> Result<()> {
        self.stream_as(source, sink, DataEncoding::Armored)
    }

    pub fn split_stream(self, source: impl Read, data_sink: impl Write) -> Result<SplitKeys> {
        let mut source = IoCapture::new(source);
        let mut sink = IoCapture::new(data_sink);
        let encrypted = self
            .encryptor()
            .encrypt_stream_split(&mut source, &mut sink);
        if let Some(error) = source.take_error().or(sink.take_error()) {
            return Err(error);
        }

        let (key_packets, info) = encrypted?;
        let session_key = info
            .revealed_session_key()
            .cloned()
            .ok_or(Error::Invalid("encryptor returned no session key"))?;
        Ok(SplitKeys::new(
            key_packets,
            SessionKey::from_inner(session_key),
        ))
    }

    pub fn session_key(self) -> Result<SessionKey> {
        let session_key = self.encryptor().generate_session_key()?;
        Ok(SessionKey::from_inner(session_key))
    }

    pub fn key_packets(self, session_key: &SessionKey) -> Result<Vec<u8>> {
        Ok(self
            .encryptor()
            .encrypt_session_key(session_key.as_inner())?)
    }

    pub(crate) fn new(time: u64) -> Self {
        Encryption {
            recipients: Vec::new(),
            passwords: Vec::new(),
            signers: Vec::new(),
            session_key: None,
            context: None,
            utf8: false,
            time,
        }
    }

    fn encryptor(&self) -> Encryptor<'_> {
        let mut encryptor = Encryptor::default()
            .with_encryption_keys(self.recipients.iter().map(PublicKey::as_inner))
            .with_signing_keys(self.signers.iter().map(PrivateKey::as_inner))
            .with_passphrases(self.passwords.iter().map(|password| password.as_slice()))
            .at_date(CheckUnixTime::new(self.time));

        if let Some(session_key) = &self.session_key {
            encryptor = encryptor.with_session_key(session_key.as_inner());
        }
        if let Some(context) = &self.context {
            encryptor =
                encryptor.with_signature_context(proton_rpgp::SignatureContext::from(context));
        }
        if self.utf8 {
            encryptor = encryptor.as_utf8();
        }
        encryptor
    }

    fn stream_as(self, source: impl Read, sink: impl Write, encoding: DataEncoding) -> Result<()> {
        let mut source = IoCapture::new(source);
        let mut sink = IoCapture::new(sink);
        let encrypted = self
            .encryptor()
            .encrypt_stream(&mut source, encoding, &mut sink);
        if let Some(error) = source.take_error().or(sink.take_error()) {
            return Err(error);
        }
        encrypted?;
        Ok(())
    }
}

impl fmt::Debug for Encryption {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Encryption")
            .field("recipients", &self.recipients.len())
            .field("passwords", &self.passwords.len())
            .field("signers", &self.signers.len())
            .field("session_key", &self.session_key.is_some())
            .field("context", &self.context)
            .field("time", &self.time)
            .finish_non_exhaustive()
    }
}
