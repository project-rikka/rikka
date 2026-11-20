use jiff::Timestamp;
use rikka_proton_api::wire_enum;
use serde::Deserialize;

use crate::identifier::{LinkId, ShareId, VolumeId};

wire_enum! {
    pub enum VolumeState: default Active {
        Active = 1,
        Deleted = 2,
        Locked = 3,
    }
}

wire_enum! {
    pub enum VolumeType: default Regular {
        Regular = 1,
        Photo = 2,
        Organization = 3,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VolumeShare {
    #[serde(rename = "ShareID")]
    id: ShareId,
    #[serde(rename = "LinkID", default)]
    link_id: LinkId,
}

impl VolumeShare {
    #[must_use]
    pub fn id(&self) -> &ShareId {
        &self.id
    }

    #[must_use]
    pub fn root_id(&self) -> &LinkId {
        &self.link_id
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Volume {
    #[serde(rename = "VolumeID")]
    id: VolumeId,
    #[serde(rename = "Share")]
    share: VolumeShare,
    #[serde(rename = "State", default)]
    state: VolumeState,
    #[serde(rename = "Type", default)]
    kind: VolumeType,
    #[serde(rename = "UsedSpace", default)]
    used_space: i64,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    created_at: Option<Timestamp>,
}

impl Volume {
    #[must_use]
    pub fn id(&self) -> &VolumeId {
        &self.id
    }

    #[must_use]
    pub fn share(&self) -> &VolumeShare {
        &self.share
    }

    #[must_use]
    pub fn state(&self) -> VolumeState {
        self.state
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == VolumeState::Active
    }

    #[must_use]
    pub fn kind(&self) -> VolumeType {
        self.kind
    }

    #[must_use]
    pub fn used_space(&self) -> i64 {
        self.used_space
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }
}
