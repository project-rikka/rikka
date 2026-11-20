use std::fmt;

use rikka_proton_pgp::{Fingerprint, PublicKey};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::key_flags::AddressKeyFlags;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedKeyListItem {
    #[serde(rename = "Primary")]
    primary: u8,
    #[serde(rename = "Flags")]
    flags: AddressKeyFlags,
    #[serde(rename = "Fingerprint")]
    fingerprint: String,
    #[serde(rename = "SHA256Fingerprints")]
    sha256_fingerprints: Vec<String>,
}

impl SignedKeyListItem {
    #[must_use]
    pub fn new(key: &PublicKey, primary: bool, flags: AddressKeyFlags) -> SignedKeyListItem {
        SignedKeyListItem {
            primary: u8::from(primary),
            flags,
            fingerprint: key.fingerprint().as_str().to_ascii_lowercase(),
            sha256_fingerprints: sha256_fingerprints(key),
        }
    }

    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.primary == 1
    }

    #[must_use]
    pub fn flags(&self) -> AddressKeyFlags {
        self.flags
    }

    #[must_use]
    pub fn is_compromised(&self) -> bool {
        !self.flags.can_verify()
    }

    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    #[must_use]
    pub fn sha256_fingerprints(&self) -> &[String] {
        &self.sha256_fingerprints
    }

    #[must_use]
    pub fn matches_key(&self, key: &PublicKey) -> bool {
        same_fingerprints(&self.sha256_fingerprints, &sha256_fingerprints(key))
    }

    pub(crate) fn with_primary(mut self, primary: bool) -> SignedKeyListItem {
        self.primary = u8::from(primary);
        self
    }

    pub(crate) fn with_flags(mut self, flags: AddressKeyFlags) -> SignedKeyListItem {
        self.flags = flags;
        self
    }

    fn difference(&self, signed: &SignedKeyListItem) -> Option<KeyDifference> {
        if !self.fingerprint.eq_ignore_ascii_case(&signed.fingerprint) {
            return Some(KeyDifference::Fingerprint);
        }
        if !same_fingerprints(&self.sha256_fingerprints, &signed.sha256_fingerprints) {
            return Some(KeyDifference::Sha256Fingerprints);
        }
        if self.flags != signed.flags {
            return Some(KeyDifference::Flags);
        }
        if self.primary != signed.primary {
            return Some(KeyDifference::Primary);
        }
        None
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyList(Vec<SignedKeyListItem>);

impl KeyList {
    pub fn parse(data: &str) -> Result<KeyList> {
        let items: Vec<SignedKeyListItem> =
            serde_json::from_str(data).map_err(|_| Error::MalformedKeyList)?;

        match items.iter().all(|item| item.primary <= 1) {
            true => Ok(KeyList(items)),
            false => Err(Error::MalformedKeyList),
        }
    }

    #[must_use]
    pub fn new(items: impl IntoIterator<Item = SignedKeyListItem>) -> KeyList {
        KeyList(items.into_iter().collect())
    }

    #[must_use]
    pub fn items(&self) -> &[SignedKeyListItem] {
        &self.0
    }

    #[must_use]
    pub fn into_items(self) -> Vec<SignedKeyListItem> {
        self.0
    }

    #[must_use]
    pub fn primary_fingerprint(&self) -> Option<&str> {
        self.0.first().map(SignedKeyListItem::fingerprint)
    }

    #[must_use]
    pub fn find(&self, key: &PublicKey) -> Option<&SignedKeyListItem> {
        self.0.iter().find(|item| item.matches_key(key))
    }

    #[must_use]
    pub fn to_data(&self) -> String {
        serde_json::to_string(&self.0).expect("key list items serialize to JSON")
    }

    pub fn compare(&self, listed: &[SignedKeyListItem]) -> Result<(), KeyListMismatch> {
        if listed.is_empty() {
            return Err(KeyListMismatch::NoKeys);
        }
        if listed.len() != self.0.len() {
            return Err(KeyListMismatch::Count {
                listed: listed.len(),
                signed: self.0.len(),
            });
        }

        let mut listed = listed.to_vec();
        let mut signed = self.0.clone();
        listed.sort_by_key(|item| item.fingerprint.to_ascii_lowercase());
        signed.sort_by_key(|item| item.fingerprint.to_ascii_lowercase());

        listed
            .iter()
            .zip(&signed)
            .find_map(|(listed, signed)| {
                listed
                    .difference(signed)
                    .map(|difference| KeyListMismatch::Key {
                        fingerprint: listed.fingerprint.clone(),
                        difference,
                    })
            })
            .map_or(Ok(()), Err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyListMismatch {
    NoKeys,
    Count {
        listed: usize,
        signed: usize,
    },
    Key {
        fingerprint: String,
        difference: KeyDifference,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyDifference {
    Fingerprint,
    Sha256Fingerprints,
    Flags,
    Primary,
}

impl fmt::Display for KeyDifference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            KeyDifference::Fingerprint => "fingerprint",
            KeyDifference::Sha256Fingerprints => "SHA-256 fingerprints",
            KeyDifference::Flags => "flags",
            KeyDifference::Primary => "primary mark",
        })
    }
}

pub(crate) fn sha256_fingerprints(key: &PublicKey) -> Vec<String> {
    key.sha256_fingerprints()
        .into_iter()
        .map(Fingerprint::into_string)
        .collect()
}

fn same_fingerprints(first: &[String], second: &[String]) -> bool {
    first.len() == second.len()
        && first
            .iter()
            .zip(second)
            .all(|(first, second)| first.eq_ignore_ascii_case(second))
}
