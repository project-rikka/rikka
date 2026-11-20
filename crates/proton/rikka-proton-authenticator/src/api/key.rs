use rikka_proton_account::UserKeys;
use rikka_proton_cache::Wired;
use rikka_proton_pgp::Pgp;
use serde::{Deserialize, Serialize};

use crate::crypto::AuthenticatorKey;
use crate::error::Result;

#[derive(Clone, Debug)]
pub struct SyncKey {
    id: String,
    user_key_id: String,
    key: AuthenticatorKey,
}

impl SyncKey {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn user_key_id(&self) -> &str {
        &self.user_key_id
    }

    #[must_use]
    pub fn key(&self) -> &AuthenticatorKey {
        &self.key
    }

    pub(crate) fn decrypt(payload: KeyPayload, pgp: &Pgp, keys: &UserKeys) -> Result<SyncKey> {
        let key = AuthenticatorKey::unwrap(pgp, keys, &payload.key)?;

        Ok(SyncKey {
            id: payload.id,
            user_key_id: payload.user_key_id,
            key,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct KeyPayload {
    #[serde(rename = "KeyID")]
    pub(crate) id: String,
    #[serde(rename = "Key")]
    pub(crate) key: String,
    #[serde(rename = "UserKeyID", default)]
    pub(crate) user_key_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct KeysPage {
    #[serde(rename = "Keys", default)]
    pub(crate) keys: Vec<Wired<KeyPayload>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateKeyRequest {
    #[serde(rename = "Key")]
    pub(crate) key: String,
}
