use proton_rpgp::{KeyGenerationType, KeyGenerator, UnixTime};

use crate::error::Result;
use crate::key_kind::KeyKind;
use crate::locked_key::LockedKey;
use crate::private_key::PrivateKey;

#[must_use]
#[derive(Debug, Clone)]
pub struct KeyGeneration {
    name: String,
    email: String,
    kind: KeyKind,
    time: u64,
}

impl KeyGeneration {
    pub fn user_id(mut self, name: impl Into<String>, email: impl Into<String>) -> Self {
        self.name = name.into();
        self.email = email.into();
        self
    }

    pub fn kind(mut self, kind: KeyKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_time(mut self, unix_seconds: u64) -> Self {
        self.time = unix_seconds;
        self
    }

    pub fn generate(self) -> Result<PrivateKey> {
        let key_type = match self.kind {
            KeyKind::Curve25519 => KeyGenerationType::ECC,
            KeyKind::PostQuantum => KeyGenerationType::PQC,
        };
        let key = KeyGenerator::default()
            .with_user_id(&self.name, &self.email)
            .with_key_type(key_type)
            .at_date(UnixTime::new(self.time))
            .generate()?;
        Ok(PrivateKey::from_inner(key))
    }

    pub fn generate_locked(self, passphrase: impl AsRef<[u8]>) -> Result<LockedKey> {
        self.generate()?.lock(passphrase)
    }

    pub(crate) fn new(time: u64) -> Self {
        KeyGeneration {
            name: String::new(),
            email: String::new(),
            kind: KeyKind::default(),
            time,
        }
    }
}
