use rikka_proton_api::parallel_map_each;
use rikka_proton_pgp::{PrivateKey, Secret, SecretBytes};
use serde::Serialize;
use serde_json::Value;

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::address_key_creation::uses_tokens;
use crate::device_secret::DeviceSecret;
use crate::error::{Error, Result};
use crate::identifier::KeyId;
use crate::passphrase;
use crate::password_mode::PasswordMode;
use crate::reauthenticated_request::{ReauthenticatedRequest, Verb};
use crate::reauthentication::reauthenticated;
use crate::verifier::{self, Verifier};

const PRIVATE_KEYS_PATH: &str = "core/v4/keys/private";

#[must_use]
#[derive(Debug)]
pub struct PasswordChange<'a> {
    request: ReauthenticatedRequest,
    keys: &'a AccountKeys,
    new_password: Secret,
    mode: PasswordMode,
    device_secret: Option<DeviceSecret>,
}

reauthenticated!(PasswordChange<'_>, request.reauthentication);

impl PasswordChange<'_> {
    pub fn new_password(mut self, password: impl Into<String>) -> Self {
        self.new_password = Secret::new(password);
        self
    }

    pub fn mode(mut self, mode: PasswordMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn device_secret(mut self, secret: &DeviceSecret) -> Self {
        self.device_secret = Some(secret.clone());
        self
    }

    pub fn keep_password_scope(mut self) -> Self {
        self.request.keep_password_scope();
        self
    }

    pub async fn execute(mut self) -> Result<AccountKeys> {
        let account = self.request.account().clone();
        let salted = passphrase::derive_with_new_salt(self.new_password.clone()).await?;
        let migrated = uses_tokens(self.keys);

        let user_keys = relock_keys(unlocked_user_keys(self.keys), &salted.passphrase).await?;
        if user_keys.is_empty() {
            return Err(Error::NoUserKey);
        }

        let auth = match self.mode {
            PasswordMode::One => Some(verifier::generate(&account, &self.new_password).await?),
            PasswordMode::Two => None,
        };
        let encrypted_secret = self
            .device_secret
            .as_ref()
            .map(|secret| secret.seal(&salted.passphrase))
            .transpose()?;

        let (user_keys, keys) = match migrated {
            true => (Some(user_keys), None),
            false => {
                let address_keys =
                    relock_keys(unlocked_address_keys(self.keys), &salted.passphrase).await?;
                (
                    None,
                    Some(user_keys.into_iter().chain(address_keys).collect()),
                )
            }
        };
        let body = PrivateKeysRequest {
            key_salt: salted.salt,
            user_keys,
            keys,
            auth,
            encrypted_secret,
        };
        self.request.extend(&body)?;
        self.request.send::<Value>().await?;

        account
            .unlock_with_passphrase(salted.passphrase.expose())
            .await
    }

    pub(crate) fn new(account: Account, keys: &AccountKeys) -> PasswordChange<'_> {
        PasswordChange {
            request: ReauthenticatedRequest::new(account, Verb::Put, PRIVATE_KEYS_PATH),
            keys,
            new_password: Secret::default(),
            mode: PasswordMode::default(),
            device_secret: None,
        }
    }
}

fn unlocked_user_keys(keys: &AccountKeys) -> Vec<(KeyId, PrivateKey)> {
    keys.user()
        .keys()
        .iter()
        .filter_map(|key| {
            keys.user_key(key.id())
                .map(|unlocked| (key.id().clone(), unlocked.clone()))
        })
        .collect()
}

fn unlocked_address_keys(keys: &AccountKeys) -> Vec<(KeyId, PrivateKey)> {
    keys.addresses()
        .iter()
        .flat_map(|address| address.keys())
        .filter_map(|key| {
            keys.address_key(key.id())
                .map(|unlocked| (key.id().clone(), unlocked.clone()))
        })
        .collect()
}

pub(crate) async fn relock_keys(
    keys: Vec<(KeyId, PrivateKey)>,
    passphrase: &SecretBytes,
) -> Result<Vec<RelockedKey>> {
    let passphrase = passphrase.clone();

    parallel_map_each(keys, move |(id, key)| {
        let armored = key.lock(&passphrase)?.export_armored()?;
        Ok(RelockedKey {
            id,
            private_key: armored,
        })
    })
    .await
    .into_iter()
    .collect()
}

#[derive(Debug, Serialize)]
pub(crate) struct RelockedKey {
    #[serde(rename = "ID")]
    id: KeyId,
    #[serde(rename = "PrivateKey")]
    private_key: String,
}

#[derive(Debug, Serialize)]
struct PrivateKeysRequest {
    #[serde(rename = "KeySalt")]
    key_salt: String,
    #[serde(rename = "UserKeys", skip_serializing_if = "Option::is_none")]
    user_keys: Option<Vec<RelockedKey>>,
    #[serde(rename = "Keys", skip_serializing_if = "Option::is_none")]
    keys: Option<Vec<RelockedKey>>,
    #[serde(rename = "Auth", skip_serializing_if = "Option::is_none")]
    auth: Option<Verifier>,
    #[serde(rename = "EncryptedSecret", skip_serializing_if = "Option::is_none")]
    encrypted_secret: Option<String>,
}
