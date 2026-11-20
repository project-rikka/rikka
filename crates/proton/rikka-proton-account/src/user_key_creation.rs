use rikka_proton_api::offload;
use rikka_proton_pgp::{KeyKind, KeyRing, Pgp, PrivateKey, SignatureContext};
use serde::Serialize;

use crate::account::Account;
use crate::account_keys::AccountKeys;
use crate::address::Address;
use crate::address_key_creation::uses_tokens;
use crate::error::{Error, Result};
use crate::identifier::KeyId;
use crate::new_key::NewUserKey;
use crate::organization::{ORGANIZATION_TOKEN_CONTEXT, ROLE_ORGANIZATION_ADMIN};
use crate::resealed_token::reseal_token;

const USER_KEYS_PATH: &str = "core/v4/keys/user";
const TOKENS_PATH: &str = "core/v4/keys/tokens";
const ACTIVATE_ORGANIZATION_PATH: &str = "core/v4/organizations/keys/activate";

#[must_use]
#[derive(Debug)]
pub struct UserKeyCreation<'a> {
    account: Account,
    keys: &'a AccountKeys,
    kind: KeyKind,
}

impl UserKeyCreation<'_> {
    pub fn kind(mut self, kind: KeyKind) -> Self {
        self.kind = kind;
        self
    }

    pub async fn execute(self) -> Result<AccountKeys> {
        let passphrase = self.keys.passphrase().ok_or(Error::NoPassphrase)?.clone();
        let pgp = *self.account.pgp();
        let kind = self.kind;
        let generated = {
            let passphrase = passphrase.clone();
            offload(move || NewUserKey::generate(&pgp, kind, &passphrase)).await?
        };

        self.account
            .api()
            .post(USER_KEYS_PATH)
            .json(&UserKeyRequest {
                primary: 1,
                private_key: &generated.armored,
            })
            .send_unit()
            .await?;

        if uses_tokens(self.keys)
            && let Err(error) = self.replace_address_tokens(&generated.key).await
        {
            tracing::warn!(%error, "failed to reencrypt address key tokens");
        }
        if self.keys.user().role() == ROLE_ORGANIZATION_ADMIN
            && let Err(error) = self.replace_organization_token(&generated.key).await
        {
            tracing::warn!(%error, "failed to reencrypt organization key token");
        }

        self.account
            .unlock_with_passphrase(passphrase.expose())
            .await
    }

    pub(crate) fn new(account: Account, keys: &AccountKeys) -> UserKeyCreation<'_> {
        UserKeyCreation {
            account,
            keys,
            kind: KeyKind::default(),
        }
    }

    async fn replace_address_tokens(&self, key: &PrivateKey) -> Result<()> {
        let pgp = *self.account.pgp();
        let addresses = self.keys.addresses().to_vec();
        let ring = self.keys.user_keys().clone();
        let key = key.clone();
        let tokens = offload(move || reseal_address_tokens(&pgp, &addresses, &ring, &key)).await;
        if tokens.is_empty() {
            return Ok(());
        }

        self.account
            .api()
            .put(TOKENS_PATH)
            .json(&TokensRequest { tokens })
            .send_unit()
            .await?;
        Ok(())
    }

    async fn replace_organization_token(&self, key: &PrivateKey) -> Result<()> {
        let Some(token) = self
            .account
            .passwordless_organization_token(self.keys)
            .await?
        else {
            return Ok(());
        };

        let pgp = *self.account.pgp();
        let ring = self.keys.user_keys().clone();
        let key = key.clone();
        let resealed = offload(move || {
            reseal_token(
                &pgp,
                &token,
                &ring,
                &key.public_key(),
                &key,
                Some(SignatureContext::new(ORGANIZATION_TOKEN_CONTEXT).critical(true)),
            )
        })
        .await?;

        self.account
            .api()
            .put(ACTIVATE_ORGANIZATION_PATH)
            .json(&OrganizationActivation {
                key_packet: resealed.key_packet,
                signature: resealed.signature,
            })
            .send_unit()
            .await?;
        Ok(())
    }
}

impl Account {
    pub fn create_user_key<'a>(&self, keys: &'a AccountKeys) -> UserKeyCreation<'a> {
        UserKeyCreation::new(self.clone(), keys)
    }
}

fn reseal_address_tokens(
    pgp: &Pgp,
    addresses: &[Address],
    ring: &KeyRing,
    key: &PrivateKey,
) -> Vec<ReplacedToken> {
    addresses
        .iter()
        .flat_map(|address| address.keys())
        .filter_map(|address_key| {
            let token = address_key.token()?;
            match reseal_token(pgp, token, ring, &key.public_key(), key, None) {
                Ok(resealed) => Some(ReplacedToken {
                    id: address_key.id().clone(),
                    key_packet: resealed.key_packet,
                    signature: resealed.signature,
                }),
                Err(error) => {
                    tracing::warn!(key = %address_key.id(), %error, "failed to reencrypt address key token");
                    None
                }
            }
        })
        .collect()
}

#[derive(Debug, Serialize)]
struct OrganizationActivation {
    #[serde(rename = "TokenKeyPacket")]
    key_packet: String,
    #[serde(rename = "Signature")]
    signature: String,
}

#[derive(Debug, Serialize)]
struct UserKeyRequest<'a> {
    #[serde(rename = "Primary")]
    primary: u8,
    #[serde(rename = "PrivateKey")]
    private_key: &'a str,
}

#[derive(Debug, Serialize)]
struct ReplacedToken {
    #[serde(rename = "AddressKeyID")]
    id: KeyId,
    #[serde(rename = "KeyPacket")]
    key_packet: String,
    #[serde(rename = "Signature")]
    signature: String,
}

#[derive(Debug, Serialize)]
struct TokensRequest {
    #[serde(rename = "AddressKeyTokens")]
    tokens: Vec<ReplacedToken>,
}
