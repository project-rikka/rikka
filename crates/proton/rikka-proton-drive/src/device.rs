use jiff::Timestamp;
use rikka_proton_api::wire_enum;
use serde::Deserialize;

use crate::folder::Folder;
use crate::identifier::{DeviceId, LinkId, ShareId, VolumeId};
use crate::share_keys::ShareKeys;

wire_enum! {
    pub enum DeviceType: default Linux {
        Windows = 1,
        MacOs = 2,
        Linux = 3,
    }
}

wire_enum! {
    pub enum DeviceSync: default Off {
        Off = 0,
        On = 1,
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(from = "DeviceEntryWire")]
pub struct Device {
    id: DeviceId,
    volume_id: VolumeId,
    share_id: ShareId,
    root_id: LinkId,
    kind: DeviceType,
    sync: DeviceSync,
    legacy_name: Option<String>,
    last_synced_at: Option<Timestamp>,
    created_at: Option<Timestamp>,
    modified_at: Option<Timestamp>,
}

impl Device {
    #[must_use]
    pub fn id(&self) -> &DeviceId {
        &self.id
    }

    #[must_use]
    pub fn volume_id(&self) -> &VolumeId {
        &self.volume_id
    }

    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn root_id(&self) -> &LinkId {
        &self.root_id
    }

    #[must_use]
    pub fn kind(&self) -> DeviceType {
        self.kind
    }

    #[must_use]
    pub fn sync(&self) -> DeviceSync {
        self.sync
    }

    #[must_use]
    pub fn legacy_name(&self) -> Option<&str> {
        self.legacy_name.as_deref()
    }

    #[must_use]
    pub fn last_synced_at(&self) -> Option<Timestamp> {
        self.last_synced_at
    }

    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.created_at
    }

    #[must_use]
    pub fn modified_at(&self) -> Option<Timestamp> {
        self.modified_at
    }
}

#[derive(Debug, Clone)]
pub struct OpenedDevice {
    device: Device,
    share: ShareKeys,
    root: Folder,
}

impl OpenedDevice {
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    #[must_use]
    pub fn share(&self) -> &ShareKeys {
        &self.share
    }

    #[must_use]
    pub fn root(&self) -> &Folder {
        &self.root
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.device.legacy_name().unwrap_or(self.root.name())
    }

    #[must_use]
    pub fn into_parts(self) -> (Device, ShareKeys, Folder) {
        (self.device, self.share, self.root)
    }

    pub(crate) fn new(device: Device, share: ShareKeys, root: Folder) -> OpenedDevice {
        OpenedDevice {
            device,
            share,
            root,
        }
    }
}

#[derive(Deserialize)]
struct DeviceEntryWire {
    #[serde(rename = "Device")]
    device: DeviceWire,
    #[serde(rename = "Share")]
    share: DeviceShareWire,
}

#[derive(Deserialize)]
struct DeviceWire {
    #[serde(rename = "DeviceID")]
    id: DeviceId,
    #[serde(rename = "VolumeID")]
    volume_id: VolumeId,
    #[serde(rename = "SyncState", default)]
    sync: DeviceSync,
    #[serde(rename = "Type", default)]
    kind: DeviceType,
    #[serde(
        rename = "LastSyncTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    last_synced_at: Option<Timestamp>,
    #[serde(
        rename = "CreateTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    created_at: Option<Timestamp>,
    #[serde(
        rename = "ModifyTime",
        default,
        deserialize_with = "rikka_proton_api::unix_seconds::optional::deserialize"
    )]
    modified_at: Option<Timestamp>,
}

#[derive(Deserialize)]
struct DeviceShareWire {
    #[serde(rename = "ShareID")]
    id: ShareId,
    #[serde(rename = "LinkID")]
    root_id: LinkId,
    #[serde(rename = "Name", default)]
    name: Option<String>,
}

impl From<DeviceEntryWire> for Device {
    fn from(entry: DeviceEntryWire) -> Device {
        Device {
            id: entry.device.id,
            volume_id: entry.device.volume_id,
            share_id: entry.share.id,
            root_id: entry.share.root_id,
            kind: entry.device.kind,
            sync: entry.device.sync,
            legacy_name: entry.share.name.filter(|name| !name.is_empty()),
            last_synced_at: entry.device.last_synced_at,
            created_at: entry.device.created_at,
            modified_at: entry.device.modified_at,
        }
    }
}
