use crate::armor::armor_public_key;
use crate::error::Result;
use crate::key_editing;
use crate::private_key::PrivateKey;
use crate::revocation_reason::RevocationReason;

#[must_use]
#[derive(Debug, Clone)]
pub struct KeyRevocation {
    key: PrivateKey,
    reason: RevocationReason,
    description: String,
    time: u64,
}

impl KeyRevocation {
    pub fn reason(mut self, reason: RevocationReason) -> Self {
        self.reason = reason;
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    pub fn with_time(mut self, unix_seconds: u64) -> Self {
        self.time = unix_seconds;
        self
    }

    pub fn certificate(self) -> Result<Vec<u8>> {
        let revocation = self.signature()?;
        key_editing::encode_signature(&revocation)
    }

    pub fn armored_certificate(self) -> Result<String> {
        armor_public_key(self.certificate()?)
    }

    pub fn apply(self) -> Result<PrivateKey> {
        let revocation = self.signature()?;
        let revoked = key_editing::add_revocation(self.key.as_inner(), revocation)?;
        Ok(PrivateKey::from_inner(revoked))
    }

    pub(crate) fn new(key: PrivateKey, time: u64) -> Self {
        KeyRevocation {
            key,
            reason: RevocationReason::default(),
            description: String::new(),
            time,
        }
    }

    fn signature(&self) -> Result<proton_rpgp::pgp::packet::Signature> {
        key_editing::revocation_signature(
            self.key.as_inner(),
            self.reason.code(),
            &self.description,
            self.time,
        )
    }
}
