use rikka_proton_pgp::{KeyRing, Pgp, PrivateKey, SignatureContext, VerificationContext};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const CONTEXT: &str = "key-transparency.verified-epoch.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct VerifiedEpoch {
    #[serde(rename = "EpochID")]
    epoch_id: u32,
    #[serde(rename = "Revision")]
    revision: u32,
    #[serde(rename = "SKLCreationTime")]
    signed_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SignedVerifiedEpoch {
    #[serde(rename = "Data")]
    data: String,
    #[serde(rename = "Signature")]
    signature: String,
}

impl VerifiedEpoch {
    pub(crate) fn new(epoch_id: u32, revision: u32, signed_at: u64) -> VerifiedEpoch {
        VerifiedEpoch {
            epoch_id,
            revision,
            signed_at,
        }
    }

    pub(crate) fn epoch_id(&self) -> u32 {
        self.epoch_id
    }

    pub(crate) fn revision(&self) -> u32 {
        self.revision
    }

    pub(crate) fn signed_at(&self) -> u64 {
        self.signed_at
    }

    pub(crate) fn sign(&self, pgp: &Pgp, user_key: &PrivateKey) -> Result<SignedVerifiedEpoch> {
        let data = serde_json::to_string(self).map_err(Error::State)?;
        let signature = pgp
            .sign()
            .with_key(user_key.clone())
            .with_context(SignatureContext::new(CONTEXT).critical(true))
            .as_utf8()
            .armored_detached(&data)?;
        Ok(SignedVerifiedEpoch { data, signature })
    }
}

impl SignedVerifiedEpoch {
    pub(crate) fn verify(&self, pgp: &Pgp, user_keys: &KeyRing) -> Option<VerifiedEpoch> {
        let status = pgp
            .verify()
            .with_ring(user_keys)
            .with_context(VerificationContext::new(CONTEXT).required(true))
            .detached(&self.data, &self.signature);
        if !status.is_valid() {
            tracing::warn!(%status, "verified epoch not signed by user key, likely after password reset");
            return None;
        }
        serde_json::from_str(&self.data)
            .inspect_err(|error| tracing::warn!(%error, "failed to read verified epoch"))
            .ok()
    }
}
