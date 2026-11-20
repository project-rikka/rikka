use std::cmp::Reverse;
use std::collections::HashSet;

use rikka_proton_pgp::PublicKey;

use crate::api_keys::ApiKeys;
use crate::kt::{KtAvailability, KtStatus, KtVerification};
use crate::pinned_keys::PinnedKeys;

#[derive(Debug, Clone)]
pub struct VerificationKeys {
    pub keys: Vec<PublicKey>,
    pub own_address: bool,
    pub pinned: bool,
    pub pinned_verified: Option<bool>,
    pub kt: Option<KtVerification>,
    pub errors: Vec<String>,
}

impl VerificationKeys {
    #[must_use]
    pub fn own(keys: Vec<PublicKey>) -> VerificationKeys {
        VerificationKeys {
            keys,
            own_address: true,
            pinned: false,
            pinned_verified: None,
            kt: Some(KtVerification::verified()),
            errors: Vec::new(),
        }
    }

    #[must_use]
    pub fn select(api: &ApiKeys, pinned: &PinnedKeys, kt: KtAvailability) -> VerificationKeys {
        let compromised: HashSet<_> = api
            .keys()
            .iter()
            .filter(|key| key.is_compromised())
            .map(|key| key.key().fingerprint())
            .collect();
        let trusted = |keys: Vec<PublicKey>| -> Vec<PublicKey> {
            keys.into_iter()
                .filter(|key| !compromised.contains(&key.fingerprint()))
                .collect()
        };

        let pinned_keys = pinned.preferences().pinned_keys.clone();
        let has_pinned = !pinned_keys.is_empty();
        let mut published: Vec<PublicKey> =
            api.keys().iter().map(|key| key.key().clone()).collect();
        published.sort_by_key(|key| Reverse(key.version()));

        let kt_verified = api.kt().is_some_and(|kt| kt.status == KtStatus::Verified);
        let keys = if has_pinned {
            trusted(pinned_keys)
        } else if api.is_internal()
            && (kt_verified || (kt == KtAvailability::Disabled && api.kt().is_none()))
        {
            trusted(published)
        } else {
            Vec::new()
        };

        VerificationKeys {
            keys,
            own_address: false,
            pinned: has_pinned,
            pinned_verified: pinned.signature_verified(),
            kt: api.kt(),
            errors: api.errors().to_vec(),
        }
    }
}
