use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use prost::Message;
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::local::LocalNigori;
use crate::proto::{
    EncryptedData, EncryptionKeys, NigoriKey, NigoriSpecifics, derivation, passphrase,
};
use google_nigori::{Derivation, Nigori};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassphraseType {
    Implicit,
    Keystore,
    FrozenImplicit,
    Custom,
    TrustedVault,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lock {
    Open,
    AwaitingKeystoreKeys,
    NeedsPassphrase,
    NeedsVaultKeys,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Pending {
    keybag: EncryptedData,
    keystore_token: Option<EncryptedData>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Encryption {
    keys: HashMap<String, Nigori>,
    primary: Option<String>,
    pending: Option<Pending>,
    keystore_keys: Vec<Vec<u8>>,
    passphrase_type: Option<PassphraseType>,
    derivation: Option<Derivation>,
    encrypt_everything: bool,
    keystore_migration_time: i64,
    custom_passphrase_time: i64,
}

impl PassphraseType {
    fn from_wire(wire: i32) -> Option<PassphraseType> {
        match wire {
            passphrase::IMPLICIT => Some(PassphraseType::Implicit),
            passphrase::KEYSTORE => Some(PassphraseType::Keystore),
            passphrase::FROZEN_IMPLICIT => Some(PassphraseType::FrozenImplicit),
            passphrase::CUSTOM => Some(PassphraseType::Custom),
            passphrase::TRUSTED_VAULT => Some(PassphraseType::TrustedVault),
            _ => None,
        }
    }

    const fn wire(self) -> i32 {
        match self {
            PassphraseType::Implicit => passphrase::IMPLICIT,
            PassphraseType::Keystore => passphrase::KEYSTORE,
            PassphraseType::FrozenImplicit => passphrase::FROZEN_IMPLICIT,
            PassphraseType::Custom => passphrase::CUSTOM,
            PassphraseType::TrustedVault => passphrase::TRUSTED_VAULT,
        }
    }

    #[must_use]
    pub const fn is_explicit(self) -> bool {
        matches!(
            self,
            PassphraseType::Custom | PassphraseType::FrozenImplicit
        )
    }

    fn may_become(self, next: PassphraseType) -> bool {
        if self == next {
            return true;
        }
        match self {
            PassphraseType::Implicit => true,
            PassphraseType::Keystore => {
                matches!(next, PassphraseType::Custom | PassphraseType::TrustedVault)
            }
            PassphraseType::FrozenImplicit | PassphraseType::TrustedVault => {
                next == PassphraseType::Custom
            }
            PassphraseType::Custom => false,
        }
    }
}

impl Encryption {
    pub(crate) fn passphrase_type(&self) -> Option<PassphraseType> {
        self.passphrase_type
    }

    pub(crate) fn lock(&self) -> Lock {
        if self.pending.is_none() {
            return match self.passphrase_type {
                Some(_) => Lock::Open,
                None => Lock::AwaitingKeystoreKeys,
            };
        }
        match self.passphrase_type {
            Some(PassphraseType::Keystore) | None => Lock::AwaitingKeystoreKeys,
            Some(PassphraseType::TrustedVault) => Lock::NeedsVaultKeys,
            Some(_) => Lock::NeedsPassphrase,
        }
    }

    pub(crate) fn is_initialized(&self) -> bool {
        self.passphrase_type.is_some()
    }

    pub(crate) fn needs_keystore_keys(&self) -> bool {
        self.keystore_keys.is_empty()
    }

    pub(crate) fn primary(&self) -> Option<&str> {
        match self.pending {
            Some(_) => None,
            None => self.primary.as_deref(),
        }
    }

    pub(crate) fn uses_explicit_passphrase(&self) -> bool {
        self.passphrase_type
            .is_some_and(PassphraseType::is_explicit)
    }

    pub(crate) fn apply(&mut self, specifics: &NigoriSpecifics) -> Result<()> {
        let keybag = specifics
            .encryption_keybag
            .clone()
            .filter(|keybag| keybag.blob.as_deref().is_some_and(|blob| !blob.is_empty()))
            .ok_or(Error::Protocol("nigori without a keybag"))?;
        let next = PassphraseType::from_wire(specifics.passphrase_type.unwrap_or(1))
            .ok_or(Error::Protocol("unknown passphrase type"))?;
        let encrypt_everything = specifics.encrypt_everything.unwrap_or(false);
        let derivation = match next {
            PassphraseType::Custom => Some(custom_derivation(specifics)?),
            _ => None,
        };
        if next.is_explicit() && !encrypt_everything {
            return Err(Error::Protocol(
                "explicit passphrase without encrypt everything",
            ));
        }
        if self
            .passphrase_type
            .is_some_and(|current| !current.may_become(next))
        {
            return Err(Error::Protocol("invalid passphrase type transition"));
        }
        if self.encrypt_everything && !encrypt_everything {
            return Err(Error::Protocol("encrypt everything was turned off"));
        }

        self.passphrase_type = Some(next);
        self.encrypt_everything = encrypt_everything;
        self.derivation = derivation;
        self.keystore_migration_time = specifics.keystore_migration_time.unwrap_or(0);
        self.custom_passphrase_time = specifics.custom_passphrase_time.unwrap_or(0);
        self.pending = Some(Pending {
            keybag,
            keystore_token: specifics.keystore_decryptor_token.clone().filter(|token| {
                next == PassphraseType::Keystore
                    && token.blob.as_deref().is_some_and(|blob| !blob.is_empty())
            }),
        });
        self.settle_with_known_keys()
    }

    pub(crate) fn set_keystore_keys(&mut self, keys: &[Vec<u8>]) -> Result<()> {
        if keys.is_empty() || keys == self.keystore_keys {
            return Ok(());
        }
        self.keystore_keys = keys.to_vec();
        self.settle_with_known_keys()
    }

    pub(crate) fn offer_passphrase(&mut self, passphrase: &str) -> Result<bool> {
        if self.pending.is_none() || self.lock() != Lock::NeedsPassphrase {
            return Ok(false);
        }
        let derivation = self.derivation.clone().unwrap_or(Derivation::Pbkdf2);
        let offered = Nigori::derive(&derivation, passphrase.as_bytes());
        self.settle(vec![offered])
    }

    pub(crate) fn offer_vault_keys(&mut self, keys: &[Vec<u8>]) -> Result<bool> {
        if self.pending.is_none() || self.lock() != Lock::NeedsVaultKeys {
            return Ok(false);
        }
        let offered = keys.iter().map(|key| password_key(key)).collect();
        self.settle(offered)
    }

    pub(crate) fn can_decrypt(&self, sealed: &EncryptedData) -> bool {
        sealed
            .key_name
            .as_deref()
            .is_some_and(|name| self.keys.contains_key(name))
    }

    pub(crate) fn decrypt(&self, sealed: &EncryptedData) -> Option<Zeroizing<Vec<u8>>> {
        let key = self.keys.get(sealed.key_name.as_deref()?)?;
        key.decrypt(sealed.blob.as_deref()?)
    }

    pub(crate) fn encrypt(&self, plain: &[u8]) -> Option<EncryptedData> {
        let name = self.primary()?;
        let key = self.keys.get(name)?;
        Some(EncryptedData {
            key_name: Some(name.to_owned()),
            blob: Some(key.encrypt(plain)),
        })
    }

    pub(crate) fn initialize_with_keystore(&mut self, now: i64) -> Option<NigoriSpecifics> {
        if self.passphrase_type.is_some() || self.keystore_keys.is_empty() {
            return None;
        }
        let mut last = None;
        for key in &self.keystore_keys {
            let key = password_key(key);
            let name = key.name();
            self.keys.insert(name.clone(), key);
            last = Some(name);
        }
        self.primary = last;
        self.passphrase_type = Some(PassphraseType::Keystore);
        self.keystore_migration_time = now;
        self.specifics()
    }

    pub(crate) fn specifics(&self) -> Option<NigoriSpecifics> {
        let primary = self.keys.get(self.primary()?)?;
        let keybag = EncryptionKeys {
            key: self.keys.values().map(export).collect(),
            cross_user_sharing_private_key: Vec::new(),
        };
        let passphrase_type = self.passphrase_type?;
        let keystore_decryptor_token = match passphrase_type {
            PassphraseType::Keystore => {
                let keystore = password_key(self.keystore_keys.last()?);
                Some(EncryptedData {
                    key_name: Some(keystore.name()),
                    blob: Some(keystore.encrypt(&export(primary).encode_to_vec())),
                })
            }
            _ => None,
        };
        let (method, salt) = match (&self.derivation, passphrase_type) {
            (Some(Derivation::Pbkdf2), PassphraseType::Custom) => {
                (Some(derivation::PBKDF2_HMAC_SHA1_1003), None)
            }
            (Some(Derivation::Scrypt { salt }), PassphraseType::Custom) => (
                Some(derivation::SCRYPT_8192_8_11),
                Some(STANDARD.encode(salt)),
            ),
            _ => (None, None),
        };
        Some(NigoriSpecifics {
            encryption_keybag: self.encrypt(&keybag.encode_to_vec()),
            keybag_is_frozen: Some(true),
            encrypt_everything: Some(self.encrypt_everything),
            passphrase_type: Some(passphrase_type.wire()),
            keystore_decryptor_token,
            keystore_migration_time: Some(self.keystore_migration_time).filter(|time| *time != 0),
            custom_passphrase_time: Some(self.custom_passphrase_time).filter(|time| *time != 0),
            custom_passphrase_key_derivation_method: method,
            custom_passphrase_key_derivation_salt: salt,
            trusted_vault_debug_info: None,
            cross_user_sharing_public_key: None,
        })
    }

    pub(crate) fn clear(&mut self) {
        *self = Encryption::default();
    }

    pub(crate) fn to_local(&self) -> LocalNigori {
        LocalNigori {
            keys: self.keys.values().map(export).collect(),
            primary: self.primary.clone(),
            pending_keybag: self.pending.as_ref().map(|pending| pending.keybag.clone()),
            pending_keystore_token: self
                .pending
                .as_ref()
                .and_then(|pending| pending.keystore_token.clone()),
            keystore_keys: self.keystore_keys.clone(),
            passphrase_type: self.passphrase_type.map(PassphraseType::wire),
            scrypt_salt: match &self.derivation {
                Some(Derivation::Scrypt { salt }) => Some(salt.clone()),
                _ => None,
            },
            pbkdf2: matches!(self.derivation, Some(Derivation::Pbkdf2)),
            encrypt_everything: self.encrypt_everything,
            keystore_migration_time: self.keystore_migration_time,
            custom_passphrase_time: self.custom_passphrase_time,
        }
    }

    pub(crate) fn from_local(local: LocalNigori) -> Encryption {
        let keys = local
            .keys
            .iter()
            .filter_map(import)
            .map(|key| (key.name(), key))
            .collect();
        Encryption {
            keys,
            primary: local.primary,
            pending: local.pending_keybag.map(|keybag| Pending {
                keybag,
                keystore_token: local.pending_keystore_token,
            }),
            keystore_keys: local.keystore_keys,
            passphrase_type: local.passphrase_type.and_then(PassphraseType::from_wire),
            derivation: match (local.scrypt_salt, local.pbkdf2) {
                (Some(salt), _) => Some(Derivation::Scrypt { salt }),
                (None, true) => Some(Derivation::Pbkdf2),
                (None, false) => None,
            },
            encrypt_everything: local.encrypt_everything,
            keystore_migration_time: local.keystore_migration_time,
            custom_passphrase_time: local.custom_passphrase_time,
        }
    }

    fn settle_with_known_keys(&mut self) -> Result<()> {
        let Some(pending) = &self.pending else {
            return Ok(());
        };
        let mut candidates: Vec<Nigori> = self.keys.values().cloned().collect();
        if self.passphrase_type == Some(PassphraseType::Keystore) {
            let keystore: Vec<Nigori> = self
                .keystore_keys
                .iter()
                .map(|key| password_key(key))
                .collect();
            if let Some(token) = &pending.keystore_token {
                let unsealed = keystore
                    .iter()
                    .filter(|key| token.key_name.as_deref() == Some(key.name().as_str()))
                    .find_map(|key| key.decrypt(token.blob.as_deref()?))
                    .and_then(|plain| NigoriKey::decode(plain.as_slice()).ok())
                    .and_then(|key| import(&key));
                candidates.extend(unsealed);
            }
            candidates.extend(keystore);
        }
        self.settle(candidates).map(|_| ())
    }

    fn settle(&mut self, candidates: Vec<Nigori>) -> Result<bool> {
        let Some(pending) = &self.pending else {
            return Ok(false);
        };
        let Some(name) = pending.keybag.key_name.as_deref() else {
            return Err(Error::Protocol("keybag without a key name"));
        };
        let opened = candidates
            .iter()
            .filter(|key| key.name() == name)
            .find_map(|key| key.decrypt(pending.keybag.blob.as_deref()?))
            .and_then(|plain| EncryptionKeys::decode(plain.as_slice()).ok());
        let Some(opened) = opened else {
            return Ok(false);
        };
        let keys: HashMap<String, Nigori> = opened
            .key
            .iter()
            .filter_map(import)
            .map(|key| (key.name(), key))
            .collect();
        if !keys.contains_key(name) {
            return Err(Error::Protocol("keybag misses its own key"));
        }
        self.primary = Some(name.to_owned());
        self.keys.extend(keys);
        self.pending = None;
        Ok(true)
    }
}

fn password_key(raw: &[u8]) -> Nigori {
    Nigori::derive(&Derivation::Pbkdf2, STANDARD.encode(raw).as_bytes())
}

fn custom_derivation(specifics: &NigoriSpecifics) -> Result<Derivation> {
    match specifics
        .custom_passphrase_key_derivation_method
        .unwrap_or(derivation::UNSPECIFIED)
    {
        derivation::UNSPECIFIED | derivation::PBKDF2_HMAC_SHA1_1003 => Ok(Derivation::Pbkdf2),
        derivation::SCRYPT_8192_8_11 => {
            let salt = specifics
                .custom_passphrase_key_derivation_salt
                .as_deref()
                .and_then(|salt| STANDARD.decode(salt).ok())
                .ok_or(Error::Protocol("scrypt passphrase without a salt"))?;
            Ok(Derivation::Scrypt { salt })
        }
        _ => Err(Error::Protocol("unknown key derivation method")),
    }
}

fn import(key: &NigoriKey) -> Option<Nigori> {
    Nigori::import(key.encryption_key.as_deref()?, key.mac_key.as_deref()?)
}

fn export(key: &Nigori) -> NigoriKey {
    NigoriKey {
        encryption_key: Some(key.encryption_key().to_vec()),
        mac_key: Some(key.mac_key().to_vec()),
    }
}
