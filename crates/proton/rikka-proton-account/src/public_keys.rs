use rikka_proton_pgp::{Pgp, PublicKey};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::key_flags::AddressKeyFlags;
use crate::key_list::SignedKeyListItem;
use crate::signed_key_list::{SignedKeyList, SignedKeyListCheck};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedKey {
    #[serde(rename = "PublicKey")]
    armored: String,
    #[serde(rename = "Flags", default)]
    flags: i32,
    #[serde(rename = "Primary", default, with = "rikka_proton_api::flag")]
    primary: bool,
    #[serde(rename = "Source", default, skip_serializing_if = "Option::is_none")]
    source: Option<i32>,
}

impl PublishedKey {
    #[must_use]
    pub fn armored(&self) -> &str {
        &self.armored
    }

    #[must_use]
    pub fn flags(&self) -> i32 {
        self.flags
    }

    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.primary
    }

    #[must_use]
    pub fn source(&self) -> Option<i32> {
        self.source
    }

    #[must_use]
    pub fn can_encrypt(&self) -> bool {
        AddressKeyFlags::from_bits(self.flags).can_encrypt()
    }

    #[must_use]
    pub fn can_verify(&self) -> bool {
        AddressKeyFlags::from_bits(self.flags).can_verify()
    }

    pub fn key(&self) -> Result<PublicKey> {
        Ok(PublicKey::import(&self.armored)?)
    }

    #[must_use]
    pub fn signed_key_list_item(&self) -> Option<SignedKeyListItem> {
        PublicKey::import(&self.armored).ok().map(|key| {
            SignedKeyListItem::new(&key, self.primary, AddressKeyFlags::from_bits(self.flags))
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedKeys {
    #[serde(rename = "Keys", default)]
    keys: Vec<PublishedKey>,
    #[serde(
        rename = "SignedKeyList",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    signed_key_list: Option<SignedKeyList>,
}

impl PublishedKeys {
    #[must_use]
    pub fn keys(&self) -> &[PublishedKey] {
        &self.keys
    }

    #[must_use]
    pub fn primary(&self) -> Option<&PublishedKey> {
        self.keys
            .iter()
            .find(|key| key.is_primary())
            .or_else(|| self.keys.first())
    }

    #[must_use]
    pub fn signed_key_list(&self) -> Option<&SignedKeyList> {
        self.signed_key_list.as_ref()
    }

    #[must_use]
    pub fn signed_key_list_items(&self) -> Vec<SignedKeyListItem> {
        self.keys
            .iter()
            .filter_map(PublishedKey::signed_key_list_item)
            .collect()
    }

    pub fn verifying_keys(&self) -> impl Iterator<Item = PublicKey> + '_ {
        self.keys
            .iter()
            .filter(|key| key.can_verify())
            .filter_map(|key| key.key().ok())
    }

    #[must_use]
    pub fn check_signed_key_list(&self, pgp: &Pgp) -> SignedKeyListCheck {
        let Some(list) = &self.signed_key_list else {
            return SignedKeyListCheck::Absent;
        };
        let keys: Vec<PublicKey> = self.keys.iter().filter_map(|key| key.key().ok()).collect();

        list.check(pgp, &keys)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicKeys {
    #[serde(rename = "Address", default)]
    address: PublishedKeys,
    #[serde(rename = "CatchAll", default, skip_serializing_if = "Option::is_none")]
    catch_all: Option<PublishedKeys>,
    #[serde(
        rename = "Unverified",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    unverified: Option<PublishedKeys>,
    #[serde(rename = "Warnings", default)]
    warnings: Vec<String>,
    #[serde(rename = "ProtonMX", default)]
    proton_mx: bool,
    #[serde(rename = "IsProton", default)]
    is_proton: i32,
}

impl PublicKeys {
    #[must_use]
    pub fn address(&self) -> &PublishedKeys {
        &self.address
    }

    #[must_use]
    pub fn catch_all(&self) -> Option<&PublishedKeys> {
        self.catch_all.as_ref()
    }

    #[must_use]
    pub fn unverified(&self) -> Option<&PublishedKeys> {
        self.unverified.as_ref()
    }

    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    #[must_use]
    pub fn has_proton_mx(&self) -> bool {
        self.proton_mx
    }

    #[must_use]
    pub fn is_proton(&self) -> bool {
        self.is_proton != 0
    }
}
