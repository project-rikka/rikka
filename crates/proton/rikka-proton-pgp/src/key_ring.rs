use crate::error::Result;
use crate::identifier::Fingerprint;
use crate::locked_key::LockedKey;
use crate::private_key::PrivateKey;
use crate::public_key::PublicKey;

#[derive(Debug, Clone, Default)]
pub struct KeyRing {
    public: Vec<PublicKey>,
    private: Vec<PrivateKey>,
}

impl KeyRing {
    #[must_use]
    pub fn new() -> Self {
        KeyRing::default()
    }

    #[must_use]
    pub fn with_public(mut self, key: PublicKey) -> Self {
        self.public.push(key);
        self
    }

    #[must_use]
    pub fn with_public_keys(mut self, keys: impl IntoIterator<Item = PublicKey>) -> Self {
        self.public.extend(keys);
        self
    }

    #[must_use]
    pub fn with_private(mut self, key: PrivateKey) -> Self {
        self.private.push(key);
        self
    }

    #[must_use]
    pub fn with_private_keys(mut self, keys: impl IntoIterator<Item = PrivateKey>) -> Self {
        self.private.extend(keys);
        self
    }

    pub fn unlock(
        keys: impl IntoIterator<Item = LockedKey>,
        passphrase: impl AsRef<[u8]>,
    ) -> Result<Self> {
        let mut ring = KeyRing::new();
        for key in keys {
            ring.private.push(key.unlock(passphrase.as_ref())?);
        }
        Ok(ring)
    }

    #[must_use]
    pub fn public_keys(&self) -> &[PublicKey] {
        &self.public
    }

    #[must_use]
    pub fn private_keys(&self) -> &[PrivateKey] {
        &self.private
    }

    #[must_use]
    pub fn find_public(&self, fingerprint: &Fingerprint) -> Option<&PublicKey> {
        self.public.iter().find(|key| {
            key.fingerprints()
                .iter()
                .any(|listed| listed == fingerprint)
        })
    }

    #[must_use]
    pub fn find_private(&self, fingerprint: &Fingerprint) -> Option<&PrivateKey> {
        self.private
            .iter()
            .find(|key| &key.fingerprint() == fingerprint)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.public.len() + self.private.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn all_public_keys(&self) -> Vec<PublicKey> {
        self.public
            .iter()
            .cloned()
            .chain(self.private.iter().map(PrivateKey::public_key))
            .collect()
    }
}
