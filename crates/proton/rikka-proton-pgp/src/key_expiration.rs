use crate::error::Result;
use crate::key_editing;
use crate::private_key::PrivateKey;

#[must_use]
#[derive(Debug, Clone)]
pub struct KeyExpiration {
    key: PrivateKey,
    expires_at: Option<u64>,
    time: u64,
}

impl KeyExpiration {
    pub fn expires_at(mut self, unix_seconds: u64) -> Self {
        self.expires_at = Some(unix_seconds);
        self
    }

    pub fn never_expires(mut self) -> Self {
        self.expires_at = None;
        self
    }

    pub fn with_time(mut self, unix_seconds: u64) -> Self {
        self.time = unix_seconds;
        self
    }

    pub fn apply(self) -> Result<PrivateKey> {
        let changed = key_editing::set_expiration(self.key.as_inner(), self.expires_at, self.time)?;
        Ok(PrivateKey::from_inner(changed))
    }

    pub(crate) fn new(key: PrivateKey, time: u64) -> Self {
        KeyExpiration {
            key,
            expires_at: None,
            time,
        }
    }
}
