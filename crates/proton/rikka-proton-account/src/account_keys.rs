use std::collections::HashMap;
use std::fmt;

use rikka_proton_api::{parallel_map, parallel_map_each};
use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey, PublicKey, SecretBytes};

use crate::address::Address;
use crate::address_key::AddressKey;
use crate::error::{Error, Result};
use crate::fork_payload::ForkPayload;
use crate::identifier::{AddressId, KeyId};
use crate::key_flags::AddressKeyFlags;
use crate::profile::Profile;
use crate::unlock;
use crate::user::User;
use crate::user_key::UserKey;
use crate::user_keys::UserKeys;

#[derive(Clone)]
pub struct AccountKeys {
    user: User,
    addresses: Vec<Address>,
    user_ring: KeyRing,
    address_rings: HashMap<AddressId, KeyRing>,
    user_keys: HashMap<KeyId, PrivateKey>,
    address_keys: HashMap<KeyId, PrivateKey>,
    primary_user_key: Option<KeyId>,
    passphrase: Option<SecretBytes>,
}

impl AccountKeys {
    #[must_use]
    pub fn user(&self) -> &User {
        &self.user
    }

    #[must_use]
    pub fn addresses(&self) -> &[Address] {
        &self.addresses
    }

    #[must_use]
    pub fn address(&self, id: &AddressId) -> Option<&Address> {
        self.addresses.iter().find(|address| address.id() == id)
    }

    #[must_use]
    pub fn address_by_email(&self, email: &str) -> Option<&Address> {
        self.addresses
            .iter()
            .find(|address| address.email().eq_ignore_ascii_case(email))
    }

    pub fn primary_address(&self) -> Result<&Address> {
        self.addresses
            .iter()
            .find(|address| address.is_enabled() && self.address_rings.contains_key(address.id()))
            .or_else(|| self.addresses.first())
            .ok_or(Error::NoAddress)
    }

    pub fn sending_addresses(&self) -> impl Iterator<Item = &Address> {
        self.addresses.iter().filter(|address| {
            address.can_send()
                && address.is_enabled()
                && self.address_rings.contains_key(address.id())
        })
    }

    #[must_use]
    pub fn user_keys(&self) -> &KeyRing {
        &self.user_ring
    }

    #[must_use]
    pub fn user_key(&self, id: &KeyId) -> Option<&PrivateKey> {
        self.user_keys.get(id)
    }

    #[must_use]
    pub fn primary_user_key(&self) -> Option<&PrivateKey> {
        self.primary_user_key
            .as_ref()
            .and_then(|id| self.user_keys.get(id))
    }

    #[must_use]
    pub fn address_keys(&self, address: &AddressId) -> Option<&KeyRing> {
        self.address_rings.get(address)
    }

    #[must_use]
    pub fn address_key(&self, id: &KeyId) -> Option<&PrivateKey> {
        self.address_keys.get(id)
    }

    #[must_use]
    pub fn signing_key(&self, address: &AddressId) -> Option<&PrivateKey> {
        self.address(address)?
            .primary_key()
            .and_then(|key| self.address_keys.get(key.id()))
            .or_else(|| self.address_rings.get(address)?.private_keys().first())
    }

    pub fn known_address(&self, id: &AddressId) -> Result<&Address> {
        self.address(id)
            .ok_or_else(|| Error::UnknownAddress(id.clone()))
    }

    pub fn address_ring(&self, address: &AddressId) -> Result<&KeyRing> {
        self.address_keys(address)
            .ok_or_else(|| Error::NoAddressKey(address.clone()))
    }

    pub fn signer(&self, address: &AddressId) -> Result<&PrivateKey> {
        self.known_address(address)?;
        self.signing_key(address)
            .ok_or_else(|| Error::NoAddressKey(address.clone()))
    }

    #[must_use]
    pub fn verification_keys(&self, address: &AddressId) -> Vec<PublicKey> {
        self.address(address)
            .map(|address| {
                address
                    .keys()
                    .iter()
                    .filter(|key| AddressKeyFlags::from_bits(key.flags()).can_verify())
                    .filter_map(|key| self.address_keys.get(key.id()))
                    .map(PrivateKey::public_key)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn key(&self, id: &KeyId) -> Option<&PrivateKey> {
        self.user_keys.get(id).or_else(|| self.address_keys.get(id))
    }

    #[must_use]
    pub fn public_key(&self, id: &KeyId) -> Option<PublicKey> {
        self.key(id).map(PrivateKey::public_key)
    }

    #[must_use]
    pub fn unlocked_key_count(&self) -> usize {
        self.user_keys.len() + self.address_keys.len()
    }

    pub fn user_signing(&self) -> Result<UserKeys> {
        let signer = self.primary_user_key().ok_or(Error::NoUserKey)?.clone();
        Ok(UserKeys::new(self.user_ring.clone(), signer))
    }

    pub(crate) fn address_rings(&self) -> impl Iterator<Item = &KeyRing> {
        self.addresses
            .iter()
            .filter_map(|address| self.address_rings.get(address.id()))
    }

    #[must_use]
    pub fn fork_payload(&self) -> Option<ForkPayload> {
        self.passphrase.as_ref().map(|passphrase| {
            ForkPayload::new().with_key_password(String::from_utf8_lossy(passphrase.expose()))
        })
    }

    #[must_use]
    pub fn profile(&self) -> Profile {
        Profile::new(self.user.clone(), self.addresses.clone())
    }

    pub(crate) fn with_profile(&self, profile: Profile) -> AccountKeys {
        let (user, addresses) = profile.into_parts();
        AccountKeys {
            user,
            addresses,
            ..self.clone()
        }
    }

    pub(crate) fn passphrase(&self) -> Option<&SecretBytes> {
        self.passphrase.as_ref()
    }

    pub(crate) async fn open(
        pgp: Pgp,
        profile: Profile,
        passphrase: Option<SecretBytes>,
    ) -> Result<AccountKeys> {
        let (user, addresses) = profile.into_parts();
        let unlocked_user = unlock_user_keys(&user, passphrase.clone()).await;
        if !user.keys().is_empty() && unlocked_user.keys.is_empty() {
            return Err(Error::NoKeyUnlocked);
        }
        let unlocked_addresses =
            unlock_address_keys(pgp, &addresses, &unlocked_user.ring, passphrase.clone()).await;
        warn_untrusted_key_lists(pgp, &addresses).await;

        Ok(AccountKeys {
            user,
            addresses,
            user_ring: unlocked_user.ring,
            address_rings: unlocked_addresses.rings,
            user_keys: unlocked_user.keys,
            address_keys: unlocked_addresses.keys,
            primary_user_key: unlocked_user.primary,
            passphrase,
        })
    }
}

struct UnlockedUserKeys {
    ring: KeyRing,
    keys: HashMap<KeyId, PrivateKey>,
    primary: Option<KeyId>,
}

struct UnlockedAddressKeys {
    rings: HashMap<AddressId, KeyRing>,
    keys: HashMap<KeyId, PrivateKey>,
}

async fn unlock_user_keys(user: &User, passphrase: Option<SecretBytes>) -> UnlockedUserKeys {
    let attempts = match passphrase {
        Some(secret) => {
            let active: Vec<UserKey> = user
                .keys()
                .iter()
                .filter(|key| key.is_active())
                .cloned()
                .collect();
            parallel_map_each(active, move |key| {
                let unlocked = unlock::user_key(&key, &secret);
                (key, unlocked)
            })
            .await
        }
        None => Vec::new(),
    };

    let mut unlocked_user = UnlockedUserKeys {
        ring: KeyRing::new(),
        keys: HashMap::new(),
        primary: None,
    };
    for (key, unlocked) in attempts {
        match unlocked {
            Ok(unlocked) => {
                if unlocked_user.primary.is_none() || key.is_primary() {
                    unlocked_user.primary = Some(key.id().clone());
                }
                unlocked_user.ring = unlocked_user.ring.with_private(unlocked.clone());
                unlocked_user.keys.insert(key.id().clone(), unlocked);
            }
            Err(error) => {
                tracing::warn!(key = %key.id(), %error, "failed to unlock user key");
            }
        }
    }
    unlocked_user
}

async fn unlock_address_keys(
    pgp: Pgp,
    addresses: &[Address],
    user_ring: &KeyRing,
    passphrase: Option<SecretBytes>,
) -> UnlockedAddressKeys {
    let active: Vec<(AddressId, AddressKey)> = addresses
        .iter()
        .flat_map(|address| {
            address
                .keys()
                .iter()
                .filter(|key| key.is_active())
                .map(|key| (address.id().clone(), key.clone()))
        })
        .collect();
    let ring = user_ring.clone();
    let attempts = parallel_map_each(active, move |(address, key)| {
        let unlocked = unlock::address_key(&pgp, &key, &ring, passphrase.as_ref());
        (address, key, unlocked)
    })
    .await;

    let mut unlocked_addresses = UnlockedAddressKeys {
        rings: HashMap::new(),
        keys: HashMap::new(),
    };
    for (address, key, unlocked) in attempts {
        match unlocked {
            Ok(unlocked) => {
                let ring = unlocked_addresses
                    .rings
                    .remove(&address)
                    .unwrap_or_default();
                unlocked_addresses
                    .rings
                    .insert(address, ring.with_private(unlocked.clone()));
                unlocked_addresses.keys.insert(key.id().clone(), unlocked);
            }
            Err(error) => {
                tracing::warn!(
                    address = %address,
                    key = %key.id(),
                    %error,
                    "failed to unlock address key"
                );
            }
        }
    }
    unlocked_addresses
}

async fn warn_untrusted_key_lists(pgp: Pgp, addresses: &[Address]) {
    let checks = parallel_map(addresses.to_vec(), move |address| {
        let check = address.check_signed_key_list(&pgp);
        (address, check)
    })
    .await;
    for (address, check) in checks {
        if !check.is_trustworthy() {
            tracing::warn!(address = %address.id(), %check, "address signed key list mismatch");
        }
    }
}

impl fmt::Debug for AccountKeys {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccountKeys")
            .field("user", self.user.id())
            .field("addresses", &self.addresses.len())
            .field("user_keys", &self.user_keys.len())
            .field("address_keys", &self.address_keys.len())
            .finish_non_exhaustive()
    }
}
