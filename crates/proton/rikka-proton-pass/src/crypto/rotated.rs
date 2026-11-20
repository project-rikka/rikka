use crate::crypto::key::EncryptionKey;

#[derive(Debug, Clone)]
pub(crate) struct RotatedKey {
    pub(crate) rotation: i64,
    pub(crate) key: EncryptionKey,
}

macro_rules! rotated_key {
    ($name:ident) => {
        #[derive(Debug, Clone)]
        pub struct $name {
            rotation: i64,
            key: EncryptionKey,
        }

        impl $name {
            #[must_use]
            pub fn new(rotation: i64, key: EncryptionKey) -> $name {
                $name { rotation, key }
            }

            #[must_use]
            pub fn rotation(&self) -> i64 {
                self.rotation
            }

            #[must_use]
            pub fn key(&self) -> &EncryptionKey {
                &self.key
            }
        }

        impl From<$name> for RotatedKey {
            fn from(key: $name) -> RotatedKey {
                RotatedKey {
                    rotation: key.rotation,
                    key: key.key,
                }
            }
        }
    };
}

rotated_key!(ShareKey);
rotated_key!(ItemKey);
rotated_key!(FolderKey);

#[derive(Debug, Clone, Default)]
pub struct ShareKeys(Vec<ShareKey>);

impl ShareKeys {
    #[must_use]
    pub fn new(keys: Vec<ShareKey>) -> ShareKeys {
        ShareKeys(keys)
    }

    #[must_use]
    pub fn get(&self, rotation: i64) -> Option<&ShareKey> {
        self.0.iter().find(|key| key.rotation() == rotation)
    }

    #[must_use]
    pub fn latest(&self) -> Option<&ShareKey> {
        self.0.iter().max_by_key(|key| key.rotation())
    }

    #[must_use]
    pub fn keys(&self) -> &[ShareKey] {
        &self.0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
