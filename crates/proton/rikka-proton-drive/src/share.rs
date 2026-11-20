use jiff::Timestamp;
use rikka_proton_account::AddressId;
use rikka_proton_api::wire_enum;
use serde::Deserialize;

use crate::identifier::{LinkId, ShareId, VolumeId};
use crate::membership::Membership;

wire_enum! {
    pub enum ShareType: default Main {
        Main = 1,
        Standard = 2,
        Device = 3,
        Photo = 4,
        Organization = 5,
    }
}

wire_enum! {
    pub enum ShareState: default Active {
        Active = 1,
        Deleted = 2,
        Restored = 3,
        Locked = 6,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Share {
    #[serde(rename = "ShareID")]
    id: ShareId,
    #[serde(rename = "VolumeID")]
    volume_id: VolumeId,
    #[serde(rename = "LinkID")]
    link_id: LinkId,
    #[serde(rename = "Type", default)]
    kind: ShareType,
    #[serde(rename = "State", default)]
    state: ShareState,
    #[serde(rename = "Creator", default)]
    creator: Option<String>,
    #[serde(rename = "AddressID", default)]
    address_id: Option<AddressId>,
    #[serde(rename = "Locked", default)]
    locked: Option<bool>,
    #[serde(rename = "EditorsCanShare", default)]
    editors_can_share: bool,
    #[serde(rename = "Memberships", default)]
    memberships: Vec<Membership>,
    #[serde(rename = "Flags", default)]
    flags: i64,
    #[serde(rename = "Key", default)]
    key: Option<String>,
    #[serde(rename = "Passphrase", default)]
    passphrase: Option<String>,
    #[serde(rename = "PassphraseSignature", default)]
    passphrase_signature: Option<String>,
    #[serde(
        rename = "CreationTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    created_at: Option<Timestamp>,
}

impl Share {
    #[must_use]
    pub fn id(&self) -> &ShareId {
        &self.id
    }

    #[must_use]
    pub fn volume_id(&self) -> &VolumeId {
        &self.volume_id
    }

    #[must_use]
    pub fn root_id(&self) -> &LinkId {
        &self.link_id
    }

    #[must_use]
    pub fn kind(&self) -> ShareType {
        self.kind
    }

    #[must_use]
    pub fn state(&self) -> ShareState {
        self.state
    }

    #[must_use]
    pub fn creator(&self) -> Option<&str> {
        self.creator.as_deref()
    }

    #[must_use]
    pub fn address_id(&self) -> Option<&AddressId> {
        self.address_id.as_ref()
    }

    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked.unwrap_or(false)
    }

    #[must_use]
    pub fn editors_can_share(&self) -> bool {
        self.editors_can_share
    }

    #[must_use]
    pub fn memberships(&self) -> &[Membership] {
        &self.memberships
    }

    #[must_use]
    pub fn flags(&self) -> i64 {
        self.flags
    }

    #[must_use]
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    #[must_use]
    pub fn passphrase(&self) -> Option<&str> {
        self.passphrase.as_deref()
    }

    #[must_use]
    pub fn passphrase_signature(&self) -> Option<&str> {
        self.passphrase_signature.as_deref()
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }
}
