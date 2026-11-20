use std::fmt;

use rikka_proton_pgp::{PrivateKey, SessionKey};

use crate::hash_key::HashKey;
use rikka_proton_pgp::Secret;

#[derive(Clone)]
pub struct NodeKeys {
    key: PrivateKey,
    passphrase: Secret,
    passphrase_session_key: SessionKey,
    name_session_key: SessionKey,
    hash_key: Option<HashKey>,
    content_key: Option<SessionKey>,
}

impl NodeKeys {
    #[must_use]
    pub fn key(&self) -> &PrivateKey {
        &self.key
    }

    #[must_use]
    pub fn hash_key(&self) -> Option<&HashKey> {
        self.hash_key.as_ref()
    }

    #[must_use]
    pub fn content_key(&self) -> Option<&SessionKey> {
        self.content_key.as_ref()
    }

    #[must_use]
    pub fn hash(&self, name: &str) -> Option<String> {
        self.hash_key.as_ref().map(|key| key.hash(name))
    }

    pub(crate) fn new(
        key: PrivateKey,
        passphrase: Secret,
        passphrase_session_key: SessionKey,
        name_session_key: SessionKey,
    ) -> NodeKeys {
        NodeKeys {
            key,
            passphrase,
            passphrase_session_key,
            name_session_key,
            hash_key: None,
            content_key: None,
        }
    }

    pub(crate) fn with_hash_key(mut self, hash_key: HashKey) -> NodeKeys {
        self.hash_key = Some(hash_key);
        self
    }

    pub(crate) fn with_content_key(mut self, content_key: SessionKey) -> NodeKeys {
        self.content_key = Some(content_key);
        self
    }

    pub(crate) fn passphrase(&self) -> &Secret {
        &self.passphrase
    }

    pub(crate) fn passphrase_session_key(&self) -> &SessionKey {
        &self.passphrase_session_key
    }

    pub(crate) fn name_session_key(&self) -> &SessionKey {
        &self.name_session_key
    }
}

impl fmt::Debug for NodeKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeKeys")
            .field("key", &self.key.fingerprint())
            .field("hash_key", &self.hash_key.is_some())
            .field("content_key", &self.content_key.is_some())
            .finish_non_exhaustive()
    }
}
