use std::fmt;

use proton_rpgp::pgp::packet::Signature;
use proton_rpgp::{VerificationError, VerificationInformation, VerificationResult};

use crate::identifier::KeyId;
use crate::public_key::PublicKey;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignatureStatus {
    Valid {
        key_id: KeyId,
        created_at: u64,
    },
    Invalid {
        key_id: Option<KeyId>,
        reason: String,
    },
    Expired {
        key_id: KeyId,
        created_at: u64,
    },
    BadContext {
        key_id: Option<KeyId>,
        reason: String,
    },
    UnknownKey {
        key_id: Option<KeyId>,
    },
    Missing,
}

impl SignatureStatus {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        matches!(self, SignatureStatus::Valid { .. })
    }

    #[must_use]
    pub fn is_missing(&self) -> bool {
        matches!(self, SignatureStatus::Missing)
    }

    #[must_use]
    pub fn key_id(&self) -> Option<&KeyId> {
        match self {
            SignatureStatus::Valid { key_id, .. } | SignatureStatus::Expired { key_id, .. } => {
                Some(key_id)
            }
            SignatureStatus::Invalid { key_id, .. }
            | SignatureStatus::BadContext { key_id, .. }
            | SignatureStatus::UnknownKey { key_id } => key_id.as_ref(),
            SignatureStatus::Missing => None,
        }
    }

    #[must_use]
    pub fn created_at(&self) -> Option<u64> {
        match self {
            SignatureStatus::Valid { created_at, .. }
            | SignatureStatus::Expired { created_at, .. } => Some(*created_at),
            _ => None,
        }
    }

    pub(crate) fn from_verification(
        result: VerificationResult,
        keys: &[PublicKey],
        time: u64,
    ) -> SignatureStatus {
        match result {
            Ok(information) => SignatureStatus::Valid {
                key_id: information.key_id.into(),
                created_at: information.signature_creation_time.unix_seconds(),
            },
            Err(VerificationError::NotSigned) => SignatureStatus::Missing,
            Err(VerificationError::NoVerifier(information, _)) => {
                let key_id = KeyId::from(information.key_id);
                if has_expired_key(keys, &key_id, time) {
                    SignatureStatus::expired(&information)
                } else {
                    SignatureStatus::UnknownKey {
                        key_id: Some(key_id),
                    }
                }
            }
            Err(VerificationError::Failed(information, reason)) => {
                if is_signature_expired(&information.signature, time) {
                    SignatureStatus::expired(&information)
                } else {
                    SignatureStatus::Invalid {
                        key_id: Some(information.key_id.into()),
                        reason,
                    }
                }
            }
            Err(VerificationError::BadContext(information, reason)) => {
                SignatureStatus::BadContext {
                    key_id: Some(information.key_id.into()),
                    reason,
                }
            }
            Err(VerificationError::RuntimeError(reason)) => SignatureStatus::Invalid {
                key_id: None,
                reason,
            },
        }
    }

    fn expired(information: &VerificationInformation) -> SignatureStatus {
        SignatureStatus::Expired {
            key_id: information.key_id.into(),
            created_at: information.signature_creation_time.unix_seconds(),
        }
    }
}

impl fmt::Display for SignatureStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignatureStatus::Valid { key_id, .. } => write!(formatter, "signed by {key_id}"),
            SignatureStatus::Invalid { reason, .. } => {
                write!(formatter, "signature verification failed: {reason}")
            }
            SignatureStatus::Expired { key_id, .. } => {
                write!(formatter, "signature by {key_id} expired")
            }
            SignatureStatus::BadContext { reason, .. } => {
                write!(formatter, "signature context mismatch: {reason}")
            }
            SignatureStatus::UnknownKey { key_id } => match key_id {
                Some(key_id) => write!(formatter, "no verification key for {key_id}"),
                None => formatter.write_str("no verification key"),
            },
            SignatureStatus::Missing => formatter.write_str("missing signature"),
        }
    }
}

fn has_expired_key(keys: &[PublicKey], key_id: &KeyId, time: u64) -> bool {
    keys.iter()
        .filter(|key| key.key_ids().contains(key_id))
        .any(|key| key.is_expired_at(time))
}

fn is_signature_expired(signature: &Signature, time: u64) -> bool {
    let Some(created) = signature.created() else {
        return false;
    };
    let Some(lifetime) = signature.signature_expiration_time() else {
        return false;
    };
    if lifetime.as_secs() == 0 {
        return false;
    }
    u64::from(created.as_secs()).saturating_add(u64::from(lifetime.as_secs())) < time
}
