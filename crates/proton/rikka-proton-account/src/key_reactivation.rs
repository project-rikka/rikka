use std::collections::BTreeMap;

use rikka_proton_api::offload;
use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey, Secret, SecretBytes};
use serde::Serialize;

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::active_keys::ActiveKeys;
use crate::address::Address;
use crate::address_key_creation::uses_tokens;
use crate::address_key_token::AddressKeyToken;
use crate::error::{Error, Result};
use crate::identifier::{AddressId, KeyId};
use crate::key_flags::AddressKeyFlags;
use crate::new_key::{USER_KEY_USER_ID, replace_legacy_user_id};
use crate::passphrase;
use crate::signed_key_list::SignedKeyList;
use crate::unlock;

#[must_use]
#[derive(Debug)]
pub struct KeyReactivation<'a> {
    account: Account,
    keys: &'a AccountKeys,
    key: KeyId,
    old_password: Secret,
}

impl KeyReactivation<'_> {
    pub fn old_password(mut self, password: impl Into<String>) -> Self {
        self.old_password = Secret::new(password);
        self
    }

    pub async fn execute(self) -> Result<AccountKeys> {
        let passphrase = self.keys.passphrase().ok_or(Error::NoPassphrase)?.clone();

        if self.keys.key(&self.key).is_some() {
            return Err(Error::KeyAlreadyActive(self.key));
        }

        let salts = self.account.key_salts().await?;
        let salt = salts
            .iter()
            .find(|salt| salt.id() == &self.key)
            .ok_or_else(|| Error::NoKeySalt(self.key.clone()))?
            .bytes()?;
        let old_passphrase = passphrase::derive(self.old_password.clone(), salt).await?;

        match self.keys.user().key(&self.key) {
            Some(_) => {
                self.reactivate_user_key(&passphrase, old_passphrase)
                    .await?
            }
            None => self.reactivate_address_key(old_passphrase).await?,
        }

        self.account
            .unlock_with_passphrase(passphrase.expose())
            .await
    }

    async fn reactivate_user_key(
        &self,
        passphrase: &SecretBytes,
        old_passphrase: SecretBytes,
    ) -> Result<()> {
        let inactive = self
            .keys
            .user()
            .key(&self.key)
            .ok_or_else(|| Error::NoSuchKey(self.key.clone()))?
            .clone();
        let addresses = self.account.addresses().await?;

        let pgp = *self.account.pgp();
        let keys = self.keys.clone();
        let id = self.key.clone();
        let passphrase = passphrase.clone();
        let reactivation_request = offload(move || {
            let unlocked = unlock::user_key(&inactive, &old_passphrase)
                .map_err(|_| Error::KeyNotReactivated(id.clone()))?;
            let unlocked = replace_legacy_user_id(unlocked, USER_KEY_USER_ID)?;
            let armored = unlocked.lock(&passphrase)?.export_armored()?;
            let ring = keys.user_keys().clone().with_private(unlocked);

            let mut fingerprints = Vec::new();
            let mut signed_key_lists = BTreeMap::new();
            for address in &addresses {
                let reactivated =
                    reactivatable_address_keys(&pgp, address, &keys, &ring, &passphrase);
                if reactivated.is_empty() {
                    continue;
                }
                let list = reactivated.into_iter().fold(
                    ActiveKeys::for_address(address, &keys),
                    |list, (key_id, key, flags)| {
                        fingerprints.push(key.fingerprint().into_string());
                        list.with_key(key_id, key, flags, false)
                    },
                );
                signed_key_lists.insert(address.id().clone(), list.sign(&pgp)?);
            }

            Ok::<_, Error>(UserKeyRequest {
                private_key: armored,
                address_key_fingerprints: fingerprints,
                signed_key_lists,
            })
        })
        .await?;

        self.account
            .api()
            .put(format!("core/v4/keys/user/{}", self.key))
            .json(&reactivation_request)
            .send_unit()
            .await?;

        Ok(())
    }

    async fn reactivate_address_key(&self, old_passphrase: SecretBytes) -> Result<()> {
        let address = self
            .account
            .address_holding_key(self.keys, &self.key)
            .await?;
        let inactive = address
            .key(&self.key)
            .ok_or_else(|| Error::NoSuchKey(self.key.clone()))?
            .clone();
        let user_key = self
            .keys
            .primary_user_key()
            .ok_or(Error::NoUserKey)?
            .clone();

        let pgp = *self.account.pgp();
        let keys = self.keys.clone();
        let id = self.key.clone();
        let reactivation_request = offload(move || {
            let unlocked = inactive
                .to_locked_key()?
                .unlock(&old_passphrase)
                .map_err(|_| Error::KeyNotReactivated(id.clone()))?;
            let unlocked = replace_legacy_user_id(unlocked, address.email())?;
            let token = AddressKeyToken::generate(&pgp, &user_key)?;
            let armored = unlocked.lock(token.secret.expose())?.export_armored()?;
            let flags = AddressKeyFlags::from(inactive.flags()).for_reactivation();
            let signed_key_list = ActiveKeys::for_address(&address, &keys)
                .with_key(id, unlocked, flags, false)
                .sign(&pgp)?;

            Ok::<_, Error>(TokenKeyRequest::new(armored, token, signed_key_list))
        })
        .await?;

        self.account
            .api()
            .put(format!("core/v4/keys/address/{}", self.key))
            .json(&reactivation_request)
            .send_unit()
            .await?;

        Ok(())
    }

    pub(crate) fn new(account: Account, keys: &AccountKeys, key: KeyId) -> KeyReactivation<'_> {
        KeyReactivation {
            account,
            keys,
            key,
            old_password: Secret::default(),
        }
    }
}

fn reactivatable_address_keys(
    pgp: &Pgp,
    address: &Address,
    keys: &AccountKeys,
    ring: &KeyRing,
    passphrase: &SecretBytes,
) -> Vec<(KeyId, PrivateKey, AddressKeyFlags)> {
    address
        .keys()
        .iter()
        .filter(|key| key.is_migrated() && keys.address_key(key.id()).is_none())
        .filter_map(|key| {
            let unlocked = unlock::address_key(pgp, key, ring, Some(passphrase)).ok()?;
            let flags = AddressKeyFlags::from(key.flags()).for_reactivation();
            Some((key.id().clone(), unlocked, flags))
        })
        .collect()
}

impl Account {
    pub fn reactivate_key<'a>(&self, keys: &'a AccountKeys, key: &KeyId) -> KeyReactivation<'a> {
        KeyReactivation::new(self.clone(), keys, key.clone())
    }

    pub async fn activate_address_keys(&self, keys: &AccountKeys) -> Result<usize> {
        let relock = match uses_tokens(keys) {
            true => Relock::Token(keys.primary_user_key().ok_or(Error::NoUserKey)?.clone()),
            false => Relock::Passphrase(keys.passphrase().ok_or(Error::NoPassphrase)?.clone()),
        };
        let mut activated = 0;

        for cached_address in keys.addresses() {
            let pending: Vec<(KeyId, PrivateKey)> = cached_address
                .keys()
                .iter()
                .filter(|key| key.activation().is_some())
                .filter_map(|key| {
                    keys.address_key(key.id())
                        .map(|unlocked| (key.id().clone(), unlocked.clone()))
                })
                .collect();
            if pending.is_empty() {
                continue;
            }
            let address = self.address(cached_address.id()).await?;

            for (id, unlocked) in pending {
                let pgp = *self.pgp();
                let list = ActiveKeys::for_address(&address, keys);
                let relock = relock.clone();
                let activation =
                    offload(move || build_activation(&pgp, &list, &unlocked, &relock)).await?;

                let request = match &activation {
                    Activation::Token(body) => self
                        .api()
                        .put(format!("core/v4/keys/address/{id}"))
                        .json(body),
                    Activation::Legacy(body) => self
                        .api()
                        .put(format!("core/v4/keys/{id}/activate"))
                        .json(body),
                };
                request.send_unit().await?;
                activated += 1;
            }
        }

        Ok(activated)
    }
}

#[derive(Clone)]
enum Relock {
    Token(PrivateKey),
    Passphrase(SecretBytes),
}

enum Activation {
    Token(TokenKeyRequest),
    Legacy(LegacyKeyRequest),
}

fn build_activation(
    pgp: &Pgp,
    list: &ActiveKeys,
    unlocked: &PrivateKey,
    relock: &Relock,
) -> Result<Activation> {
    let signed_key_list = list.sign(pgp)?;

    match relock {
        Relock::Token(user_key) => {
            let token = AddressKeyToken::generate(pgp, user_key)?;
            let armored = unlocked.lock(token.secret.expose())?.export_armored()?;
            Ok(Activation::Token(TokenKeyRequest::new(
                armored,
                token,
                signed_key_list,
            )))
        }
        Relock::Passphrase(passphrase) => Ok(Activation::Legacy(LegacyKeyRequest {
            private_key: unlocked.lock(passphrase)?.export_armored()?,
            signed_key_list,
        })),
    }
}

#[derive(Debug, Serialize)]
struct UserKeyRequest {
    #[serde(rename = "PrivateKey")]
    private_key: String,
    #[serde(rename = "AddressKeyFingerprints")]
    address_key_fingerprints: Vec<String>,
    #[serde(rename = "SignedKeyLists")]
    signed_key_lists: BTreeMap<AddressId, SignedKeyList>,
}

#[derive(Debug, Serialize)]
struct TokenKeyRequest {
    #[serde(rename = "PrivateKey")]
    private_key: String,
    #[serde(rename = "SignedKeyList")]
    signed_key_list: SignedKeyList,
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "Signature")]
    signature: String,
}

impl TokenKeyRequest {
    fn new(
        private_key: String,
        token: AddressKeyToken,
        signed_key_list: SignedKeyList,
    ) -> TokenKeyRequest {
        TokenKeyRequest {
            private_key,
            signed_key_list,
            token: token.encrypted,
            signature: token.signature,
        }
    }
}

#[derive(Debug, Serialize)]
struct LegacyKeyRequest {
    #[serde(rename = "PrivateKey")]
    private_key: String,
    #[serde(rename = "SignedKeyList")]
    signed_key_list: SignedKeyList,
}
