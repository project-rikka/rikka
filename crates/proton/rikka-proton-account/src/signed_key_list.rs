use std::fmt;

use rikka_proton_pgp::{
    Pgp, PrivateKey, PublicKey, SignatureContext, SignatureStatus, VerificationContext,
};
use serde::{Deserialize, Serialize};

use crate::active_keys::{ActiveKey, ActiveKeys};
use crate::error::{Error, Result};
use crate::key_list::{KeyList, SignedKeyListItem, sha256_fingerprints};

pub const SIGNED_KEY_LIST_CONTEXT: &str = "key-transparency.key-list";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedKeyList {
    #[serde(rename = "Data", default, skip_serializing_if = "Option::is_none")]
    data: Option<String>,
    #[serde(rename = "Signature", default, skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(
        rename = "MinEpochID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    min_epoch_id: Option<u32>,
    #[serde(
        rename = "MaxEpochID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    max_epoch_id: Option<u32>,
    #[serde(
        rename = "ExpectedMinEpochID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    expected_min_epoch_id: Option<u32>,
    #[serde(
        rename = "ObsolescenceToken",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    obsolescence_token: Option<String>,
    #[serde(rename = "Revision", default, skip_serializing_if = "Option::is_none")]
    revision: Option<u32>,
}

impl SignedKeyList {
    pub fn sign(
        pgp: &Pgp,
        items: &[SignedKeyListItem],
        signer: &PrivateKey,
    ) -> Result<SignedKeyList> {
        SignedKeyList::sign_with_keys(pgp, items, std::slice::from_ref(signer))
    }

    pub fn sign_with_keys(
        pgp: &Pgp,
        items: &[SignedKeyListItem],
        signers: &[PrivateKey],
    ) -> Result<SignedKeyList> {
        let data = KeyList::new(items.iter().cloned()).to_data();
        let signature = sign_data(pgp, &data, signers, pgp.now())?;

        Ok(SignedKeyList {
            data: Some(data),
            signature: Some(signature),
            ..SignedKeyList::default()
        })
    }

    pub fn from_active_keys(pgp: &Pgp, keys: &[ActiveKey]) -> Result<SignedKeyList> {
        ActiveKeys::from_active(keys).sign(pgp)
    }

    #[must_use]
    pub fn data(&self) -> Option<&str> {
        present(self.data.as_deref())
    }

    #[must_use]
    pub fn signature(&self) -> Option<&str> {
        present(self.signature.as_deref())
    }

    #[must_use]
    pub fn min_epoch_id(&self) -> Option<u32> {
        self.min_epoch_id.filter(|id| *id != 0)
    }

    #[must_use]
    pub fn max_epoch_id(&self) -> Option<u32> {
        self.max_epoch_id.filter(|id| *id != 0)
    }

    #[must_use]
    pub fn expected_min_epoch_id(&self) -> Option<u32> {
        self.expected_min_epoch_id.filter(|id| *id != 0)
    }

    #[must_use]
    pub fn obsolescence_token(&self) -> Option<&str> {
        present(self.obsolescence_token.as_deref())
    }

    #[must_use]
    pub fn revision(&self) -> u32 {
        self.revision.unwrap_or_default()
    }

    #[must_use]
    pub fn content(&self) -> Option<&str> {
        self.data().or_else(|| self.obsolescence_token())
    }

    #[must_use]
    pub fn is_included(&self) -> bool {
        self.min_epoch_id().is_some()
    }

    #[must_use]
    pub fn is_obsolete(&self) -> bool {
        self.obsolescence_token().is_some()
    }

    #[must_use]
    pub fn key_list(&self) -> Option<KeyList> {
        KeyList::parse(self.data()?).ok()
    }

    #[must_use]
    pub fn item_for(&self, key: &PublicKey) -> Option<SignedKeyListItem> {
        self.key_list()?.find(key).cloned()
    }

    pub fn resign(&self, pgp: &Pgp, signers: &[PrivateKey], time: u64) -> Result<String> {
        let data = self.data().ok_or(Error::MalformedKeyList)?;
        sign_data(pgp, data, signers, time)
    }

    #[must_use]
    pub fn signed_at(&self, pgp: &Pgp, keys: &[PublicKey], time: u64) -> Option<u64> {
        let status = self.verify_at(pgp, keys, time);
        match status.is_valid() {
            true => status.created_at(),
            false => None,
        }
    }

    #[must_use]
    pub fn verify(&self, pgp: &Pgp, keys: &[PublicKey]) -> SignatureStatus {
        self.verify_at(pgp, keys, pgp.now())
    }

    #[must_use]
    pub fn check(&self, pgp: &Pgp, keys: &[PublicKey]) -> SignedKeyListCheck {
        if self.data().is_none() {
            return match self.is_obsolete() {
                true => SignedKeyListCheck::Obsolete,
                false => SignedKeyListCheck::Absent,
            };
        }
        if self.signature().is_none() {
            return SignedKeyListCheck::Malformed;
        }
        let Some(list) = self.key_list() else {
            return SignedKeyListCheck::Malformed;
        };

        let status = self.verify(pgp, keys);
        if !status.is_valid() {
            return SignedKeyListCheck::Unsigned(status);
        }

        let mut listed: Vec<Vec<String>> = list
            .into_items()
            .into_iter()
            .map(|item| item.sha256_fingerprints().to_vec())
            .collect();
        let mut fingerprints: Vec<Vec<String>> = keys.iter().map(sha256_fingerprints).collect();
        listed.sort();
        fingerprints.sort();

        match listed == fingerprints {
            true => SignedKeyListCheck::Verified,
            false => SignedKeyListCheck::Mismatched,
        }
    }

    fn verify_at(&self, pgp: &Pgp, keys: &[PublicKey], time: u64) -> SignatureStatus {
        let (Some(data), Some(signature)) = (self.data(), self.signature()) else {
            return SignatureStatus::Missing;
        };

        pgp.verify()
            .with_keys(keys.iter().cloned())
            .with_context(VerificationContext::new(SIGNED_KEY_LIST_CONTEXT).required(false))
            .as_utf8()
            .with_time(time)
            .detached(data, signature)
    }
}

fn sign_data(pgp: &Pgp, data: &str, signers: &[PrivateKey], time: u64) -> Result<String> {
    Ok(pgp
        .sign()
        .with_keys(signers.iter().cloned())
        .with_context(SignatureContext::new(SIGNED_KEY_LIST_CONTEXT).critical(false))
        .as_utf8()
        .with_time(time)
        .armored_detached(data)?)
}

fn present(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignedKeyListCheck {
    Absent,
    Obsolete,
    Verified,
    Mismatched,
    Malformed,
    Unsigned(SignatureStatus),
}

impl SignedKeyListCheck {
    #[must_use]
    pub fn is_trustworthy(&self) -> bool {
        matches!(
            self,
            SignedKeyListCheck::Absent
                | SignedKeyListCheck::Obsolete
                | SignedKeyListCheck::Verified
        )
    }

    #[must_use]
    pub fn is_forged(&self) -> bool {
        matches!(
            self,
            SignedKeyListCheck::Malformed | SignedKeyListCheck::Unsigned(_)
        )
    }
}

impl fmt::Display for SignedKeyListCheck {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignedKeyListCheck::Absent => formatter.write_str("no signed key list"),
            SignedKeyListCheck::Obsolete => formatter.write_str("obsolete signed key list"),
            SignedKeyListCheck::Verified => formatter.write_str("verified signed key list"),
            SignedKeyListCheck::Mismatched => {
                formatter.write_str("signed key list does not match served keys")
            }
            SignedKeyListCheck::Malformed => formatter.write_str("malformed signed key list"),
            SignedKeyListCheck::Unsigned(status) => {
                write!(formatter, "invalid signed key list signature: {status}")
            }
        }
    }
}
