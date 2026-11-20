use rikka_proton_api::wire_enum;
use rikka_proton_pgp::{Pgp, PublicKey};
use serde::{Deserialize, Serialize};

use crate::address_key::AddressKey;
use crate::identifier::{AddressId, KeyId};
use crate::signed_key_list::{SignedKeyList, SignedKeyListCheck};

const DISABLE_E2EE: i32 = 1 << 4;
const DISABLE_EXPECTED_SIGNED: i32 = 1 << 5;

wire_enum! {
    pub enum AddressStatus: default Disabled {
        Disabled = 0,
        Enabled = 1,
        Deleting = 2,
    }
}

wire_enum! {
    pub enum AddressType: default Original {
        Original = 1,
        Alias = 2,
        Custom = 3,
        Premium = 4,
        External = 5,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    #[serde(rename = "ID")]
    id: AddressId,
    #[serde(rename = "DomainID", default, skip_serializing_if = "Option::is_none")]
    domain_id: Option<String>,
    #[serde(rename = "Email")]
    email: String,
    #[serde(rename = "Send", default, with = "rikka_proton_api::flag")]
    send: bool,
    #[serde(rename = "Receive", default, with = "rikka_proton_api::flag")]
    receive: bool,
    #[serde(rename = "Status", default)]
    status: AddressStatus,
    #[serde(rename = "Type", default)]
    kind: AddressType,
    #[serde(rename = "Order", default)]
    order: i32,
    #[serde(
        rename = "DisplayName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    display_name: Option<String>,
    #[serde(rename = "Signature", default, skip_serializing_if = "Option::is_none")]
    signature: Option<String>,
    #[serde(rename = "HasKeys", default, with = "rikka_proton_api::flag")]
    has_keys: bool,
    #[serde(rename = "Keys", default)]
    keys: Vec<AddressKey>,
    #[serde(
        rename = "SignedKeyList",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    signed_key_list: Option<SignedKeyList>,
    #[serde(rename = "Flags", default, skip_serializing_if = "Option::is_none")]
    flags: Option<i32>,
}

impl Address {
    #[must_use]
    pub fn id(&self) -> &AddressId {
        &self.id
    }

    #[must_use]
    pub fn domain_id(&self) -> Option<&str> {
        self.domain_id.as_deref()
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn can_send(&self) -> bool {
        self.send
    }

    #[must_use]
    pub fn can_receive(&self) -> bool {
        self.receive
    }

    #[must_use]
    pub fn status(&self) -> AddressStatus {
        self.status
    }

    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.status == AddressStatus::Enabled
    }

    #[must_use]
    pub fn kind(&self) -> AddressType {
        self.kind
    }

    #[must_use]
    pub fn order(&self) -> i32 {
        self.order
    }

    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    #[must_use]
    pub fn signature(&self) -> Option<&str> {
        self.signature.as_deref()
    }

    #[must_use]
    pub fn has_keys(&self) -> bool {
        self.has_keys
    }

    #[must_use]
    pub fn keys(&self) -> &[AddressKey] {
        &self.keys
    }

    #[must_use]
    pub fn key(&self, id: &KeyId) -> Option<&AddressKey> {
        self.keys.iter().find(|key| key.id() == id)
    }

    #[must_use]
    pub fn primary_key(&self) -> Option<&AddressKey> {
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
    pub fn flags(&self) -> i32 {
        self.flags.unwrap_or_default()
    }

    #[must_use]
    pub fn has_end_to_end_encryption(&self) -> bool {
        self.flags() & DISABLE_E2EE == 0
    }

    #[must_use]
    pub fn expects_signed_mail(&self) -> bool {
        self.flags() & DISABLE_EXPECTED_SIGNED == 0
    }

    #[must_use]
    pub fn check_signed_key_list(&self, pgp: &Pgp) -> SignedKeyListCheck {
        let Some(list) = &self.signed_key_list else {
            return SignedKeyListCheck::Absent;
        };
        let keys: Vec<PublicKey> = self
            .keys
            .iter()
            .filter(|key| key.is_active())
            .filter_map(|key| key.to_locked_key().ok())
            .map(|locked| locked.public_key())
            .collect();

        list.check(pgp, &keys)
    }
}
